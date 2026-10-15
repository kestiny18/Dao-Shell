//! Windows index worker and fixed SCM service integration.
mod fixture;
pub mod protocol;
mod security;
pub mod service;
mod storage;
pub use storage::diagnostics as experiment_diagnostics;
pub use storage::location as experiment_storage_location;
#[cfg(test)]
mod tests;
mod transport;
use super::{
    Budget, Cancellation, IndexStore, SearchCursor, VolumeSource, windows::WindowsVolumeSource,
};
use anyhow::{Result, ensure};
use protocol::*;
use security::Principal;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone)]
pub enum Mode {
    Volume(String),
    Fixture(PathBuf),
}
pub struct Config {
    pub mode: Mode,
    pub database: PathBuf,
    pub pipe: String,
    pub max_resolutions: usize,
}
fn validate_source_location(identity: &super::VolumeIdentity, config: &Config) -> Result<()> {
    let (guid, serial) = identity
        .stable_id
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid source identity"))?;
    let key = service::source_key(guid, u32::from_str_radix(serial, 16)?)?;
    ensure!(
        config.pipe == format!("volume-{key}")
            && config
                .database
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|v| v.to_str())
                == Some(&key),
        "discovered volume identity changed"
    );
    Ok(())
}
trait WorkerSource: VolumeSource + Send {
    fn prepare(&mut self, paths: &[PathBuf]) -> Result<()>;
    fn metrics(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
impl WorkerSource for WindowsVolumeSource {
    fn metrics(&self) -> serde_json::Value {
        serde_json::to_value(&self.metrics).unwrap_or_default()
    }
    fn prepare(&mut self, paths: &[PathBuf]) -> Result<()> {
        self.exclude_paths(paths)
    }
}
impl WorkerSource for fixture::FixtureSource {
    fn prepare(&mut self, _: &[PathBuf]) -> Result<()> {
        Ok(())
    }
}
struct Shared {
    stop: Cancellation,
    db: PathBuf,
    root: PathBuf,
    state: Mutex<PublicStatus>,
    active: Mutex<HashMap<String, (Principal, Cancellation)>>,
    cursors: Mutex<HashMap<String, Cursor>>,
    rebuild: Mutex<Rebuild>,
}
#[derive(Default)]
struct Rebuild {
    disabled: bool,
    pending: bool,
    running: bool,
    last: Option<Instant>,
}
impl Rebuild {
    fn request(&mut self, now: Instant) -> &'static str {
        if self.disabled {
            return "unavailable";
        }
        if self.pending || self.running {
            return "in_progress";
        }
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(30))
        {
            return "rate_limited";
        }
        self.pending = true;
        self.last = Some(now);
        "accepted"
    }
    fn take(&mut self) -> bool {
        if !self.pending {
            return false;
        }
        self.pending = false;
        self.running = true;
        true
    }
}
fn initial() -> PublicStatus {
    PublicStatus {
        coverage_reasons: Vec::new(),
        diagnostic: None,
        phase: "building".into(),
        generation: None,
        complete: false,
        updated_unix_ms: None,
        error: None,
    }
}
fn maintenance_stopped(state: &mut PublicStatus) {
    state.phase = "error".into();
    // Preserve the actionable storage failure, but do not leave an old,
    // recoverable request error looking rebuildable after its writer exited.
    if state.error.as_deref() != Some("storage_identity_changed") {
        state.error = Some("maintenance_stopped".into());
    }
}
fn classify(error: &anyhow::Error) -> String {
    let text = error.to_string();
    if text.contains("rebuild_required") {
        "rebuild_required"
    } else if text.contains("index_not_ready") {
        "index_not_ready"
    } else if text.contains("cancel") || text.contains("interrupted") {
        "cancelled"
    } else if text.contains("stale_cursor") {
        "stale_cursor"
    } else if text.contains("budget") {
        "budget_exhausted"
    } else {
        "request_failed"
    }
    .into()
}
fn failure_diagnostic(error: &anyhow::Error) -> FailureDiagnostic {
    let stages = [
        ("metadata: delete-pending", "metadata_standard"),
        ("metadata: before", "metadata_identity"),
        ("metadata: after", "metadata_identity"),
        ("OpenFileById", "metadata_open"),
        ("parse file USN", "file_usn_parse"),
        ("file identity mismatch", "identity_check"),
        ("journal", "journal"),
        ("USN", "usn"),
        ("MFT", "enumeration"),
    ];
    let stage = stages
        .iter()
        .find_map(|(prefix, stage)| {
            error
                .chain()
                .any(|cause| cause.to_string().starts_with(prefix))
                .then_some(*stage)
        })
        .unwrap_or("index_maintenance");
    FailureDiagnostic {
        stage: stage.into(),
        code: classify(error),
        native_code: error
            .downcast_ref::<std::io::Error>()
            .and_then(|error| error.raw_os_error()),
    }
}
fn public_reasons(reasons: &[String]) -> Vec<String> {
    let mut safe = std::collections::BTreeSet::new();
    for reason in reasons {
        safe.insert(
            match reason.as_str() {
                "access_denied"
                | "sharing_violation"
                | "delete_pending"
                | "file_metadata_not_supported"
                | "invalid_id_still_in_mft"
                | "deletion_confirmation_unavailable"
                | "metadata_access_or_sharing_denied"
                | "unresolved_parent_chain" => reason.as_str(),
                _ => "metadata_unavailable",
            }
            .to_owned(),
        );
    }
    safe.into_iter().collect()
}
fn retry_delay(error: &anyhow::Error, attempts: &mut u32) -> Option<u64> {
    if !super::metadata_retryable(error) || *attempts >= 6 {
        return None;
    }
    let delay = (250u64 << *attempts).min(4000);
    *attempts += 1;
    Some(delay)
}
pub fn run(config: Config, stop: Cancellation) -> Result<PublicStatus> {
    run_internal(config, stop, None, None)
}
fn run_internal(
    config: Config,
    stop: Cancellation,
    service_owner: Option<String>,
    ready: Option<std::sync::mpsc::Sender<()>>,
) -> Result<PublicStatus> {
    let service_storage = service_owner.is_some();
    let fixture = matches!(config.mode, Mode::Fixture(_));
    ensure!(
        service_owner.is_none() || !fixture,
        "service cannot use fixture source"
    );
    ensure!(config.database.is_absolute(), "database must be absolute");
    let mut storage_guards = if fixture {
        Vec::new()
    } else if service_owner.is_some() {
        service::prepare_database(&config.database)?
    } else {
        storage::prepare(&config.database)?
    };
    let (root, source): (PathBuf, Box<dyn WorkerSource>) = match &config.mode {
        Mode::Volume(volume) => {
            let mut source = WindowsVolumeSource::open(volume)?;
            if service_storage {
                validate_source_location(&source.identity(), &config)?;
                source.checkpoint()?;
            }
            let root = PathBuf::from(source.volume_root());
            source.exclude_paths(&[
                config.database.clone(),
                PathBuf::from(format!("{}-wal", config.database.display())),
                PathBuf::from(format!("{}-shm", config.database.display())),
            ])?;
            (root, Box::new(source))
        }
        Mode::Fixture(root) => {
            ensure!(
                !security::current()?.elevated,
                "fixture worker must run without elevation"
            );
            let source = fixture::FixtureSource::open(root)?;
            (root.clone(), Box::new(source))
        }
    };
    let writer = IndexStore::open(&config.database)?;
    if !fixture {
        storage_guards.push(storage::validate_database(&config.database)?);
    }
    writer.enable_concurrent_reads()?;
    let mut owner = security::current()?;
    let pipes = if let Some(sid) = service_owner {
        ensure!(
            owner.sid == "S-1-5-18" && owner.session == 0,
            "SCM SYSTEM worker required"
        );
        owner.sid = sid;
        transport::listeners_for(&config.pipe, &owner, false)?
    } else {
        transport::listeners(&config.pipe, fixture)?
    };
    let mut startup = initial();
    if let Ok(published) = writer.status() {
        startup.phase = "catching_up".into();
        startup.generation = Some(published.generation);
        startup.complete = published.complete;
        startup.updated_unix_ms = published.published_unix_ms;
        startup.coverage_reasons = public_reasons(&published.coverage_reasons);
    }
    let shared = Arc::new(Shared {
        stop: stop.clone(),
        db: config.database,
        state: Mutex::new(startup),
        root,
        active: Mutex::new(HashMap::new()),
        cursors: Mutex::new(HashMap::new()),
        rebuild: Mutex::new(Rebuild::default()),
    });
    let maintenance = shared.clone();
    let worker_stop = stop.clone();
    let writer_thread = std::thread::spawn(move || {
        maintain(
            writer,
            source,
            maintenance,
            worker_stop,
            config.max_resolutions,
            service_storage,
        )
    });
    let mut threads = Vec::new();
    for pipe in pipes {
        let shared = shared.clone();
        let stop = stop.clone();
        let owner = owner.clone();
        threads.push(std::thread::spawn(move || {
            while !stop.cancelled() {
                if let Err(error) = transport::accept(security::raw(&pipe), &stop) {
                    if stop.cancelled() {
                        break;
                    }
                    stop.cancel();
                    return Err(error);
                }
                let _ = serve_connection(security::raw(&pipe), &shared, &owner);
                transport::disconnect(security::raw(&pipe));
            }
            Ok(())
        }));
    }
    let mut writer_observed = false;
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
    while !stop.cancelled() {
        if threads.iter().any(|thread| thread.is_finished()) {
            stop.cancel();
            break;
        }
        if writer_thread.is_finished() && !writer_observed {
            writer_observed = true;
            shared.rebuild.lock().unwrap().disabled = true;
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            maintenance_stopped(&mut state);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for (_, cancel) in shared.active.lock().unwrap().values() {
        cancel.cancel();
    }
    let mut failed = false;
    for thread in threads {
        failed |= !matches!(thread.join(), Ok(Ok(())));
    }
    failed |= writer_thread.join().is_err();
    ensure!(!failed, "worker thread failed");
    Ok(shared.state.lock().unwrap().clone())
}
fn maintain(
    mut store: IndexStore,
    mut source: Box<dyn WorkerSource>,
    shared: Arc<Shared>,
    stop: Cancellation,
    budget: usize,
    service_storage: bool,
) {
    let mut rebuild = store.status().is_err();
    let mut delay = 100;
    let mut blocked = false;
    let mut retry_attempts = 0;
    let mut diagnostics_left = 128;
    while !stop.cancelled() {
        if shared.rebuild.lock().unwrap().take() {
            rebuild = true;
            blocked = false;
            retry_attempts = 0;
            shared.state.lock().unwrap().phase = "rebuilding".into();
        }
        if !blocked {
            let mut own_paths = vec![
                shared.db.clone(),
                PathBuf::from(format!("{}-wal", shared.db.display())),
                PathBuf::from(format!("{}-shm", shared.db.display())),
                shared.db.with_file_name("worker-diagnostics.jsonl"),
            ];
            if service_storage {
                match service::storage_paths() {
                    Ok(paths) => own_paths.extend(paths),
                    Err(_) => {
                        let mut state = shared.state.lock().unwrap();
                        state.phase = "error".into();
                        state.error = Some("storage_identity_changed".into());
                        break;
                    }
                }
            }
            if source.prepare(&own_paths).is_err() {
                let mut state = shared.state.lock().unwrap();
                state.phase = "error".into();
                state.error = Some("storage_identity_changed".into());
                break;
            }
            let old = store.status().ok().map(|s| s.generation);
            let started = Instant::now();
            let result = store.update(
                source.as_mut(),
                rebuild,
                Budget {
                    max_resolutions: budget,
                    max_pages: 10000,
                },
                &stop,
            );
            if diagnostics_left > 0
                && (result.is_err() || result.as_ref().is_ok_and(|s| old != Some(s.generation)))
            {
                // Protected administrator-only diagnostics, never part of the IPC response.
                use std::io::Write;
                let diagnostic = serde_json::json!({"rebuild":rebuild,"elapsed_ms":started.elapsed().as_millis(),"metrics":source.metrics(),"generation":result.as_ref().ok().map(|s|s.generation),"complete":result.as_ref().ok().map(|s|s.complete),"error":result.as_ref().err().map(|e|format!("{e:#}"))});
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(shared.db.with_file_name("worker-diagnostics.jsonl"))
                {
                    let _ = writeln!(file, "{diagnostic}");
                }
                diagnostics_left -= 1;
            }
            match result {
                Ok(status) => {
                    shared.rebuild.lock().unwrap().running = false;
                    retry_attempts = 0;
                    rebuild = false;
                    let changed = old != Some(status.generation);
                    let mut state = shared.state.lock().unwrap();
                    state.phase = if status.complete {
                        "ready"
                    } else {
                        "coverage_incomplete"
                    }
                    .into();
                    state.complete = status.complete;
                    state.error = None;
                    state.diagnostic = None;
                    state.generation = Some(status.generation);
                    state.updated_unix_ms = status.published_unix_ms;
                    state.coverage_reasons = public_reasons(&status.coverage_reasons);
                    if changed {
                        delay = 100;
                    } else {
                        delay = (delay * 2).min(500);
                    }
                }
                Err(error) => {
                    let mut state = shared.state.lock().unwrap();
                    state.diagnostic = Some(failure_diagnostic(&error));
                    if let Some(backoff) = retry_delay(&error, &mut retry_attempts) {
                        delay = backoff;
                        state.phase = "retrying".into();
                        state.error = Some("metadata_changed_retrying".into());
                    } else {
                        let code = classify(&error);
                        state.phase = if code == "rebuild_required" {
                            "rebuild_required"
                        } else {
                            "error"
                        }
                        .into();
                        state.error = Some(if super::metadata_retryable(&error) {
                            "metadata_changed_retry_exhausted".into()
                        } else {
                            code
                        });
                        blocked = true;
                        shared.rebuild.lock().unwrap().running = false;
                    }
                }
            }
        }
        for _ in 0..delay / 25 {
            if stop.cancelled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
fn serve_connection(
    pipe: windows_sys::Win32::Foundation::HANDLE,
    shared: &Shared,
    owner: &Principal,
) -> Result<()> {
    let envelope = Cancellation::with_timeout(Duration::from_secs(5));
    let bytes = transport::receive(pipe, &envelope)?;
    let actor = if owner.session == 0 && owner.elevated {
        security::service_caller(pipe, &owner.sid)?
    } else {
        security::caller(pipe, owner)?
    };
    let request: Request = serde_json::from_slice(&bytes)?;
    ensure!(
        request.version == VERSION
            && uuid::Uuid::parse_str(&request.id).is_ok()
            && (50..=3000).contains(&request.timeout_ms),
        "invalid protocol envelope"
    );
    let cancel = Cancellation::with_timeout(Duration::from_millis(request.timeout_ms));
    let mut reply = Response {
        control: None,
        version: VERSION,
        id: request.id.clone(),
        status: shared.state.lock().unwrap().clone(),
        hits: Vec::new(),
        cursor: None,
        error: None,
    };
    match &request.operation {
        Operation::Cancel { request_id } => {
            if let Some((principal, token)) = shared.active.lock().unwrap().get(request_id)
                && principal == &actor
            {
                token.cancel();
            }
        }
        Operation::Status => (),
        Operation::Rebuild {} => {
            // Authentication above is authoritative; body cannot supply SID or paths.
            let outcome = if shared.stop.cancelled() {
                "unavailable"
            } else {
                shared.rebuild.lock().unwrap().request(Instant::now())
            };
            reply.control = Some(outcome.into());
            if outcome == "accepted" {
                reply.status.phase = "rebuild_queued".into();
            }
        }
        Operation::Search {
            query,
            page_size,
            cursor,
        } => {
            ensure!(
                !query.is_empty() && query.len() <= 1024 && (1..=50).contains(page_size),
                "invalid search bounds"
            );
            {
                let mut active = shared.active.lock().unwrap();
                ensure!(
                    !active.contains_key(&request.id) && active.len() < 3,
                    "request unavailable"
                );
                active.insert(request.id.clone(), (actor.clone(), cancel.clone()));
            }
            let result = search(
                pipe,
                shared,
                &actor,
                query,
                *page_size,
                cursor.as_deref(),
                &cancel,
            );
            shared.active.lock().unwrap().remove(&request.id);
            match result {
                Ok((hits, cursor, snapshot)) => {
                    reply.hits = hits;
                    reply.cursor = cursor;
                    reply.status.generation = Some(snapshot.generation);
                    reply.status.complete = snapshot.complete;
                    reply.status.updated_unix_ms = snapshot.published_unix_ms;
                    reply.status.coverage_reasons = public_reasons(&snapshot.coverage_reasons);
                }
                Err(error) => reply.error = Some(classify(&error)),
            }
        }
    }
    let output = serde_json::to_vec(&reply)?;
    transport::send(pipe, &output, &envelope)?;
    transport::wait_ack(pipe, &envelope)
}
fn search(
    pipe: windows_sys::Win32::Foundation::HANDLE,
    shared: &Shared,
    actor: &Principal,
    query: &str,
    page_size: usize,
    cursor: Option<&str>,
    cancel: &Cancellation,
) -> Result<(Vec<Hit>, Option<String>, super::Status)> {
    ensure!(
        shared.state.lock().unwrap().generation.is_some(),
        "index_not_ready"
    );
    let mut position = if let Some(key) = cursor {
        let mut cursors = shared.cursors.lock().unwrap();
        let cursor = cursors
            .remove(key)
            .ok_or_else(|| anyhow::anyhow!("stale_cursor"))?;
        ensure!(
            cursor.owner == *actor && cursor.query == query && cursor.expires > Instant::now(),
            "stale_cursor"
        );
        Some(cursor.cursor)
    } else {
        None
    };
    let store = IndexStore::open_reader(&shared.db)?;
    let mut hits = Vec::new();
    let mut last_visible = None;
    loop {
        cancel.check()?;
        let page = store.search(query, 100, position.clone(), cancel)?;
        let generation = page.status.generation;
        for candidate in page.items {
            cancel.check()?;
            let _guard = security::impersonate(pipe)?;
            if !security::visible(&shared.root, &candidate.components, &candidate.file_id) {
                continue;
            }
            if hits.len() == page_size {
                let mut cursors = shared.cursors.lock().unwrap();
                cursors.retain(|_, c| c.expires > Instant::now());
                ensure!(cursors.len() < 128, "cursor capacity");
                let key = uuid::Uuid::new_v4().to_string();
                cursors.insert(
                    key.clone(),
                    Cursor {
                        owner: actor.clone(),
                        query: query.into(),
                        cursor: last_visible
                            .ok_or_else(|| anyhow::anyhow!("missing continuation"))?,
                        expires: Instant::now() + Duration::from_secs(30),
                    },
                );
                return Ok((hits, Some(key), page.status));
            }
            let mut path = shared.root.clone();
            for part in &candidate.components {
                path.push(part);
            }
            hits.push(Hit {
                file_id: candidate.file_id,
                path: path.to_string_lossy().into_owned(),
            });
            last_visible = Some(SearchCursor {
                generation,
                after: candidate.row_id,
            });
        }
        position = page.next;
        if position.is_none() {
            return Ok((hits, None, page.status));
        }
    }
}
pub fn request(
    pipe: &str,
    expected_image: &Path,
    fixture: bool,
    request: &Request,
    cancel: &Cancellation,
) -> Result<Response> {
    let connection = transport::connect(pipe, expected_image, fixture, cancel)?;
    transport::send(
        security::raw(&connection),
        &serde_json::to_vec(request)?,
        cancel,
    )?;
    let reply: Response =
        serde_json::from_slice(&transport::receive(security::raw(&connection), cancel)?)?;
    ensure!(
        reply.version == VERSION && reply.id == request.id,
        "response mismatch"
    );
    transport::acknowledge(security::raw(&connection), cancel)?;
    Ok(reply)
}
