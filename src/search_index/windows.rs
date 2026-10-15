use super::*;
use crate::file_index::{
    self,
    worker::{protocol::*, service},
};
use anyhow::{Context, ensure};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetVolumeInformationW, GetVolumeNameForVolumeMountPointW,
};

pub struct WindowsServiceIndexClient {
    expected_build: String,
}
impl WindowsServiceIndexClient {
    pub fn new(expected_build: String) -> Self {
        Self { expected_build }
    }
    pub fn rebuild(&self, key: &str, cancel: &Cancellation) -> Result<serde_json::Value> {
        controlled(cancel, |control| {
            let health = service::health_controlled(control)?;
            ensure!(
                health["build"] == self.expected_build,
                "service version mismatch"
            );
            let response = service::request_volume(
                key,
                &Request {
                    version: VERSION,
                    id: uuid::Uuid::new_v4().to_string(),
                    timeout_ms: 1000,
                    operation: Operation::Rebuild {},
                },
                control,
            )?;
            Ok(
                serde_json::json!({"state":response.control.unwrap_or_else(||"unavailable".into()),"status":response.status}),
            )
        })
    }
    pub fn status(&self, cancel: &Cancellation) -> Result<serde_json::Value> {
        controlled(cancel, |control| {
            let scm = service::status()?;
            if scm["state"] == "not_installed" {
                return Ok(serde_json::json!({"state":"not_installed"}));
            }
            if scm["service"]["state"] != 4 {
                return Ok(serde_json::json!({"state":"stopped"}));
            }
            let mut catalog = service::health_controlled(control)?;
            catalog["state"] = serde_json::json!(if catalog["build"] == self.expected_build {
                "running"
            } else {
                "version_mismatch"
            });
            // Read-only v1 status remains authenticated even for an older build.
            // Search and rebuild retain their independent strict build checks.
            if let Some(volumes) = catalog["volumes"].as_array_mut() {
                for volume in volumes {
                    if control.cancelled() {
                        break;
                    }
                    if volume["worker_started"] != true {
                        continue;
                    }
                    if let Some(key) = volume["pipe"]
                        .as_str()
                        .and_then(|value| value.strip_prefix("volume-"))
                    {
                        let request = Request {
                            version: VERSION,
                            id: uuid::Uuid::new_v4().to_string(),
                            timeout_ms: 500,
                            operation: Operation::Status,
                        };
                        volume["index_status"] = match service::request_volume(
                            key, &request, control,
                        ) {
                            Ok(response) => serde_json::to_value(response.status)?,
                            Err(_) => {
                                serde_json::json!({"phase":"unavailable","complete":false,"coverage_reasons":["query_unavailable"]})
                            }
                        };
                    }
                }
            }
            Ok(catalog)
        })
    }
}
fn controlled<T>(
    cancel: &Cancellation,
    run: impl FnOnce(&file_index::Cancellation) -> Result<T>,
) -> Result<T> {
    cancel.check()?;
    let control = file_index::Cancellation::with_timeout(Duration::from_secs(3));
    let done = Arc::new(AtomicBool::new(false));
    let watching = done.clone();
    let source = cancel.clone();
    let target = control.clone();
    let watcher = std::thread::spawn(move || {
        while !watching.load(Ordering::Relaxed) {
            if source.is_cancelled() {
                target.cancel();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    struct Watcher(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);
    impl Drop for Watcher {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
            if let Some(handle) = self.1.take() {
                let _ = handle.join();
            }
        }
    }
    let watcher = Watcher(done, Some(watcher));
    let result = run(&control);
    drop(watcher);
    cancel.check()?;
    result
}
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}
fn mounted_identity(mount: &str) -> Result<(String, u32)> {
    ensure!(
        mount.len() == 3 && mount.as_bytes()[0].is_ascii_alphabetic() && mount.ends_with(":\\"),
        "unsupported mount"
    );
    let text = wide(mount);
    let mut guid = [0u16; 64];
    let mut serial = 0;
    ensure!(
        unsafe { GetVolumeNameForVolumeMountPointW(text.as_ptr(), guid.as_mut_ptr(), 64) } != 0,
        "mount identity unavailable"
    );
    ensure!(
        unsafe {
            GetVolumeInformationW(
                text.as_ptr(),
                std::ptr::null_mut(),
                0,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        } != 0,
        "mount serial unavailable"
    );
    Ok((
        String::from_utf16(
            &guid[..guid
                .iter()
                .position(|c| *c == 0)
                .context("invalid mount GUID")?],
        )?,
        serial,
    ))
}
pub(super) fn check_mapping(mount: &str, guid: &str, serial: u64) -> Result<()> {
    let (current, current_serial) = mounted_identity(mount)?;
    ensure!(
        current.eq_ignore_ascii_case(guid) && current_serial as u64 == serial,
        "volume mapping changed"
    );
    Ok(())
}
fn map_hit(
    hit: &Hit,
    volume: &service::Volume,
    resolve: impl Fn(&str) -> Result<(String, u32)>,
) -> Result<Candidate> {
    let suffix = hit
        .path
        .strip_prefix(&volume.guid)
        .context("candidate outside volume")?;
    ensure!(
        !suffix.is_empty()
            && !suffix.starts_with(['\\', '/'])
            && suffix
                .split(['\\', '/'])
                .all(|part| !part.is_empty() && !matches!(part, "." | "..") && !part.contains(':'))
            && std::path::Path::new(suffix)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
        "candidate suffix is not strictly relative"
    );
    let serial = volume.serial.context("missing volume serial")?;
    for mount in &volume.mounts {
        let Ok((guid, current_serial)) = resolve(mount) else {
            continue;
        };
        if !guid.eq_ignore_ascii_case(&volume.guid) || current_serial != serial {
            continue;
        }
        let path = PathBuf::from(mount).join(suffix);
        crate::files::validate_components(&path)?;
        ensure!(path.is_absolute(), "candidate path is not absolute");
        return Ok(Candidate {
            path,
            volume: serial as u64,
            file_id: u64::from_str_radix(&hit.file_id, 16)?,
            mount: Some((mount.clone(), volume.guid.clone())),
        });
    }
    anyhow::bail!("mount_changed_or_unavailable")
}
impl IndexClient for WindowsServiceIndexClient {
    fn search(&self, request: &Search, cancel: &Cancellation) -> Result<Page> {
        ensure!(
            request.page == 0,
            "global name search provides the first bounded page only"
        );
        controlled(cancel, |control| {
            let catalog = service::health_controlled(control)?;
            ensure!(
                catalog["build"] == self.expected_build,
                "索引服务版本不匹配，请在设置中更新"
            );
            let volumes: Vec<service::Volume> = serde_json::from_value(catalog["volumes"].clone())?;
            ensure!(volumes.len() <= 128, "invalid volume catalog");
            let mut page = Page {
                receipt: Receipt {
                    complete: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            for volume in volumes {
                cancel.check()?;
                let mut receipt = VolumeReceipt {
                    volume: volume
                        .mounts
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "未挂载卷".into()),
                    ..Default::default()
                };
                if !volume.supported || !volume.worker_started || volume.worker_error.is_some() {
                    receipt.phase = "unavailable".into();
                    receipt.reasons.push(
                        volume
                            .worker_error
                            .or(volume.reason)
                            .unwrap_or_else(|| "initializing".into()),
                    );
                    page.receipt.complete = false;
                    page.receipt.volumes.push(receipt);
                    continue;
                }
                if control.cancelled() {
                    receipt.phase = "timeout".into();
                    receipt.reasons.push("query_timeout".into());
                    page.receipt.complete = false;
                    page.receipt.volumes.push(receipt);
                    continue;
                }
                let key = volume
                    .pipe
                    .as_deref()
                    .and_then(|s| s.strip_prefix("volume-"))
                    .context("invalid volume endpoint")?;
                let input = Request {
                    version: VERSION,
                    id: uuid::Uuid::new_v4().to_string(),
                    timeout_ms: 1000,
                    operation: Operation::Search {
                        query: request.query.clone(),
                        page_size: 50,
                        cursor: None,
                    },
                };
                match service::request_volume(key, &input, control) {
                    Ok(response) => {
                        receipt.generation = response.status.generation;
                        receipt.updated_unix_ms = response.status.updated_unix_ms;
                        receipt.phase = response.status.phase;
                        receipt.complete = response.status.complete
                            && response.error.is_none()
                            && response.status.error.is_none();
                        receipt.reasons = response.status.coverage_reasons;
                        if let Some(reason) = &volume.reason {
                            receipt.reasons.push(reason.clone());
                            receipt.complete = false;
                        }
                        if let Some(error) = response.error.or(response.status.error) {
                            receipt.reasons.push(error);
                        }
                        if !receipt.complete && receipt.reasons.is_empty() {
                            receipt.reasons.push("coverage_incomplete".into());
                        }
                        page.has_more |= response.cursor.is_some();
                        for hit in response.hits {
                            match map_hit(&hit, &volume, mounted_identity) {
                                Ok(candidate) => page.candidates.push(candidate),
                                Err(_) => {
                                    receipt.complete = false;
                                    if !receipt
                                        .reasons
                                        .iter()
                                        .any(|r| r == "mount_or_candidate_changed")
                                    {
                                        receipt.reasons.push("mount_or_candidate_changed".into());
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        receipt.phase = "unavailable".into();
                        receipt.reasons.push("query_unavailable".into());
                    }
                }
                page.receipt.complete &= receipt.complete;
                page.receipt.volumes.push(receipt);
            }
            page.has_more |= page.candidates.len() > request.limit.unwrap_or(20);
            if page.receipt.volumes.is_empty() {
                page.receipt.complete = false;
                page.receipt.reasons.push("no_supported_volume".into());
            }
            Ok(page)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_serial_different_guid_never_maps_candidate() {
        let volume = service::Volume {
            guid: r"\\?\Volume{46ebe9ac-18bb-43fb-ae5a-511a8ad2f41a}\".into(),
            serial: Some(42),
            mounts: vec!["E:\\".into()],
            supported: true,
            reason: None,
            pipe: None,
            worker_started: true,
            worker_error: None,
        };
        let hit = Hit {
            file_id: "0001000000000042".into(),
            path: format!("{}fixture.txt", volume.guid),
        };
        assert!(
            map_hit(&hit, &volume, |_| Ok((
                r"\\?\Volume{56ebe9ac-18bb-43fb-ae5a-511a8ad2f41a}\".into(),
                42
            )))
            .is_err()
        );
        assert!(map_hit(&hit, &volume, |_| Ok((volume.guid.clone(), 43))).is_err());
        assert_eq!(
            map_hit(&hit, &volume, |_| Ok((volume.guid.clone(), 42)))
                .unwrap()
                .path,
            PathBuf::from(r"E:\fixture.txt")
        );
        for suffix in [
            r"\escape.txt",
            r"C:\escape.txt",
            r"..\escape.txt",
            r"dir\..\escape.txt",
            r"name:stream",
            "/escape.txt",
        ] {
            let malformed = Hit {
                file_id: hit.file_id.clone(),
                path: format!("{}{suffix}", volume.guid),
            };
            assert!(map_hit(&malformed, &volume, |_| Ok((volume.guid.clone(), 42))).is_err());
        }
    }
    #[test]
    fn client_control_observes_user_cancel() {
        let cancel = Cancellation::default();
        let trigger = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(15));
            trigger.cancel();
        });
        let now = std::time::Instant::now();
        assert!(
            controlled(&cancel, |control| {
                while !control.cancelled() {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            })
            .is_err()
        );
        thread.join().unwrap();
        assert!(now.elapsed() < Duration::from_secs(1));
    }
}
