//! Fixed Windows service lifecycle. No arbitrary service names, executable paths or commands.
mod health;
mod lifecycle;
mod lock;
mod package;
mod peer;
mod recovery;
mod runtime;
mod scm;
mod volumes;
pub const SERVICE_NAME: &str = "DaoShellFileIndex";
pub const PRODUCT_DIRECTORY: &str = "DaoShellIndexService";
pub use runtime::run;
pub use scm::status;
pub use volumes::{Volume, local_discovery};
pub(super) fn source_key(guid: &str, serial: u32) -> anyhow::Result<String> {
    volumes::key(guid, serial)
}
pub fn valid_volume_key(key: &str) -> bool {
    volumes::valid_key(key)
}

use super::security;
use anyhow::{Result, ensure};
use package::{Layout, Manifest};
use scm::{Controller, Native};
pub(super) fn prepare_database(
    database: &std::path::Path,
) -> Result<Vec<std::os::windows::io::OwnedHandle>> {
    static PREPARE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _prepare = PREPARE.lock().unwrap();
    let actor = security::current()?;
    ensure!(
        actor.sid == "S-1-5-18" && actor.session == 0,
        "SYSTEM service storage required"
    );
    let layout = Layout::fixed()?;
    let volume = database
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid volume storage"))?;
    let volumes_root = layout.data.join("volumes");
    ensure!(
        database.file_name().is_some_and(|name| name == "index.db")
            && volume.parent() == Some(volumes_root.as_path())
            && volume
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(volumes::valid_key),
        "invalid service database path"
    );
    let mut held = layout.hold_ancestors()?;
    for directory in [&layout.data, &volumes_root, volume] {
        if directory == volume && !volume.try_exists()? {
            ensure!(
                std::fs::read_dir(&volumes_root)?
                    .take(recovery::MAX_CATALOG)
                    .count()
                    < recovery::MAX_CATALOG,
                "volume storage limit reached"
            );
        }
        if directory != layout.data {
            super::storage::create_directory_reader(directory, None)?;
        }
        held.push(super::storage::hold(directory, true)?);
        super::storage::validate_acl_reader(directory, true, None)?;
    }
    super::storage::finish_storage(volume, held)
}
pub(super) fn storage_paths() -> Result<Vec<std::path::PathBuf>> {
    let layout = Layout::fixed()?;
    let _root = super::storage::hold(&layout.data, true)?;
    super::storage::validate_acl_reader(&layout.data, true, None)?;
    let volumes_root = layout.data.join("volumes");
    if !volumes_root.try_exists()? {
        return Ok(Vec::new());
    }
    let _volumes = super::storage::hold(&volumes_root, true)?;
    super::storage::validate_acl_reader(&volumes_root, true, None)?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(volumes_root)? {
        ensure!(
            paths.len() < recovery::MAX_CATALOG * 5,
            "volume storage limit reached"
        );
        let entry = entry?;
        ensure!(
            entry.file_name().to_str().is_some_and(volumes::valid_key),
            "unknown volume storage"
        );
        let directory = entry.path();
        let _guard = super::storage::hold(&directory, true)?;
        super::storage::validate_acl_reader(&directory, true, None)?;
        // Sidecars may be created or removed by SQLite between observations.
        // Include their expected names even when absent; the source binds existing IDs.
        for name in [
            "index.db",
            "index.db-wal",
            "index.db-shm",
            "index.db-journal",
            "worker-diagnostics.jsonl",
        ] {
            paths.push(directory.join(name));
        }
    }
    Ok(paths)
}
impl lifecycle::Repository for Layout {
    fn manifest(&self) -> Result<Manifest> {
        self.read_manifest()
    }
    fn save(&mut self, m: &Manifest) -> Result<()> {
        self.write_manifest(m)
    }
    fn command(&self, build: &str) -> Result<String> {
        Layout::command(self, build)
    }
    fn verify(&self, m: &Manifest, build: &str) -> Result<()> {
        self.verify_image(m, build).map(|_| ())
    }
    fn health(&self, m: &Manifest) -> Result<()> {
        health::wait(self, m)
    }
}
#[derive(Clone, Copy)]
pub enum Action {
    Install,
    Upgrade,
    Repair,
    Rollback,
    Uninstall,
    Start,
    Stop,
}
pub fn manage(
    action: Action,
    expected_sha256: Option<&str>,
    delete_index: bool,
) -> Result<serde_json::Value> {
    ensure!(
        !delete_index || matches!(action, Action::Uninstall),
        "delete-index is only valid for uninstall"
    );
    let actor = security::installation_principal()?;
    let _installer_lock = lock::installation()?;
    let mut layout = Layout::fixed()?;
    // A digest provided before elevation is mandatory even for repair.
    let payload = if matches!(action, Action::Install | Action::Upgrade | Action::Repair) {
        Some(package::payload(expected_sha256.ok_or_else(|| {
            anyhow::anyhow!("expected SHA256 required")
        })?)?)
    } else {
        None
    };
    if matches!(action, Action::Install) {
        ensure!(Native.inspect()?.is_none(), "service already installed");
        if layout.programs.try_exists()? {
            let retained = layout.read_manifest()?;
            ensure!(
                retained.uninstalled && retained.owner_sid == actor.sid,
                "existing installation target: use repair, never adopt unknown directories"
            );
            ensure!(
                layout.data.try_exists()?,
                "retained installation data root missing"
            );
        } else {
            ensure!(
                !layout.data.try_exists()?,
                "unknown retained data: installation refused"
            );
        }
        let (digest, bytes) = payload.unwrap();
        let manifest = Manifest {
            schema: 1,
            owner_sid: actor.sid,
            current: digest,
            previous: None,
            pending: true,
            recover_to: None,
            uninstalled: false,
        };
        let _held = layout.create_roots(&manifest.owner_sid)?;
        layout.compatible_databases()?;
        layout.stage(&manifest, &bytes)?;
        layout.write_manifest(&manifest)?;
        // Pending manifest remains recoverable if SCM creation/start/health fails.
        lifecycle::activate_new(&layout, &mut Native)?;
        layout.write_manifest(&Manifest {
            pending: false,
            ..manifest
        })?;
    } else {
        let manifest = layout.read_manifest()?;
        ensure!(
            actor.sid == manifest.owner_sid,
            "installation belongs to a different interactive user"
        );
        let _held = layout.create_roots(&manifest.owner_sid)?;
        match action {
            Action::Upgrade => {
                layout.compatible_databases()?;
                let (digest, bytes) = payload.unwrap();
                let next = Manifest {
                    current: digest.clone(),
                    ..manifest
                };
                layout.stage(&next, &bytes)?;
                lifecycle::switch(&mut layout, &mut Native, &digest)?;
            }
            Action::Repair => {
                layout.compatible_databases()?;
                let (digest, bytes) = payload.unwrap();
                if digest == manifest.current {
                    layout.stage(&manifest, &bytes)?;
                }
                lifecycle::repair(&mut layout, &mut Native)?;
            }
            Action::Rollback => {
                layout.compatible_databases()?;
                let previous = manifest
                    .previous
                    .ok_or_else(|| anyhow::anyhow!("no previous protected version"))?;
                lifecycle::switch(&mut layout, &mut Native, &previous)?;
            }
            Action::Uninstall => {
                lifecycle::uninstall(&layout, &mut Native)?;
                let retained = Manifest {
                    previous: None,
                    recover_to: None,
                    pending: false,
                    uninstalled: true,
                    ..manifest
                };
                layout.write_manifest(&retained)?;
                layout.remove_payloads(&retained)?;
                if delete_index {
                    layout.remove_index()?;
                }
            }
            Action::Start => lifecycle::start(&layout, &mut Native)?,
            Action::Stop => lifecycle::stop(&layout, &mut Native)?,
            Action::Install => unreachable!(),
        }
    }
    Ok(serde_json::json!({"status":"ok","index_retained":!delete_index}))
}
pub fn health() -> Result<serde_json::Value> {
    let layout = Layout::fixed()?;
    let manifest = layout.read_manifest()?;
    Ok(serde_json::to_value(health::query(&layout, &manifest)?)?)
}
pub fn health_controlled(cancel: &crate::file_index::Cancellation) -> Result<serde_json::Value> {
    let layout = Layout::fixed()?;
    let manifest = layout.read_manifest()?;
    Ok(serde_json::to_value(health::query_controlled(
        &layout, &manifest, cancel,
    )?)?)
}
pub fn payload_sha256(bytes: &[u8]) -> Result<String> {
    package::digest(bytes)
}
/// Ordinary-user client for a discovered GUID+serial key. Authentication never
/// accepts a caller-supplied executable, database path, service name or SID.
pub fn request_volume(
    volume_key: &str,
    request: &super::protocol::Request,
    cancel: &crate::file_index::Cancellation,
) -> Result<super::protocol::Response> {
    ensure!(volumes::valid_key(volume_key), "invalid volume key");
    let layout = Layout::fixed()?;
    let manifest = layout.read_manifest()?;
    if matches!(request.operation, super::protocol::Operation::Rebuild {}) {
        let current = health::query_controlled(&layout, &manifest, cancel)?;
        ensure!(
            current.volumes.iter().any(|v| v.supported
                && v.worker_started
                && v.worker_error.is_none()
                && v.pipe.as_deref() == Some(&format!("volume-{volume_key}"))),
            "volume unavailable for rebuild"
        );
    }
    let connection = super::transport::connect_unverified(&format!("volume-{volume_key}"), cancel)?;
    health::verify_server(security::raw(&connection), &layout, &manifest)?;
    super::transport::send(
        security::raw(&connection),
        &serde_json::to_vec(request)?,
        cancel,
    )?;
    let response: super::protocol::Response = serde_json::from_slice(&super::transport::receive(
        security::raw(&connection),
        cancel,
    )?)?;
    ensure!(
        response.version == super::protocol::VERSION && response.id == request.id,
        "response mismatch"
    );
    super::transport::acknowledge(security::raw(&connection), cancel)?;
    Ok(response)
}
