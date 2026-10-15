use super::super::{Config, Mode, security};
use super::{SERVICE_NAME, health, package::Layout, volumes};
use crate::file_index::Cancellation;
use anyhow::{Result, ensure};
use std::{
    ptr,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicPtr, Ordering},
    },
    time::Duration,
};
use windows_sys::Win32::{Foundation::*, System::Services::*};

static STOP: OnceLock<Cancellation> = OnceLock::new();
static STATUS: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());
fn report(state: u32, checkpoint: u32, error: u32) -> Result<()> {
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN | SERVICE_ACCEPT_PRESHUTDOWN
        } else {
            0
        },
        dwWin32ExitCode: error,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: checkpoint,
        dwWaitHint: if state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING {
            30000
        } else {
            0
        },
    };
    ensure!(
        unsafe { SetServiceStatus(STATUS.load(Ordering::Acquire), &status) } != 0,
        "service status reporting failed"
    );
    Ok(())
}
unsafe extern "system" fn control(
    code: u32,
    _: u32,
    _: *mut std::ffi::c_void,
    _: *mut std::ffi::c_void,
) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN | SERVICE_CONTROL_PRESHUTDOWN => {
            if let Some(stop) = STOP.get() {
                stop.cancel();
            }
            let _ = report(SERVICE_STOP_PENDING, 1, 0);
            NO_ERROR
        }
        SERVICE_CONTROL_INTERROGATE => NO_ERROR,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}
fn work(stop: Cancellation) -> Result<()> {
    let actor = security::current()?;
    ensure!(
        actor.sid == "S-1-5-18" && actor.session == 0,
        "LocalSystem SCM process required"
    );
    let layout = Layout::fixed()?;
    let manifest = layout.read_manifest()?;
    let _roots = layout.create_roots(&manifest.owner_sid)?;
    ensure!(
        std::env::current_exe()?.canonicalize()?
            == layout
                .verify_image(&manifest, &manifest.current)?
                .canonicalize()?,
        "SCM image differs from protected manifest"
    );
    let mut schedule = super::recovery::Catalog::default();
    schedule.discover(volumes::discover(), 0);
    let catalog = Arc::new(Mutex::new(schedule.snapshot()));
    struct Worker {
        ready: std::sync::mpsc::Receiver<()>,
        thread: std::thread::JoinHandle<Result<super::super::protocol::PublicStatus>>,
        stop: Cancellation,
    }
    let mut workers: std::collections::HashMap<String, Worker> = std::collections::HashMap::new();
    let signal = stop.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let shared_catalog = catalog.clone();
    let owner_sid = manifest.owner_sid.clone();
    let listener = std::thread::spawn(move || health::serve(manifest, shared_catalog, signal, tx));
    let started = std::time::Instant::now();
    let mut next_discovery = 5;
    let outcome = (|| -> Result<()> {
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| anyhow::anyhow!("service pipe startup timeout"))??;
        report(SERVICE_RUNNING, 0, 0)?;
        while !stop.cancelled() {
            ensure!(
                !listener.is_finished(),
                "service control listener exited unexpectedly"
            );
            let now = started.elapsed().as_secs();
            if now >= next_discovery {
                schedule.discover(volumes::discover(), now);
                next_discovery = now + 5;
            }
            for (key, worker) in &workers {
                if schedule.entries[key].stopping {
                    worker.stop.cancel();
                }
                if worker.ready.try_recv().is_ok() {
                    schedule.ready(key);
                }
            }
            let finished: Vec<_> = workers
                .iter()
                .filter(|(_, w)| w.thread.is_finished())
                .map(|(k, _)| k.clone())
                .collect();
            for key in finished {
                let worker = workers.remove(&key).unwrap();
                let reason = match worker.thread.join() {
                    Ok(Err(error)) => volume_error(&error),
                    Err(_) => "worker_panicked",
                    Ok(Ok(_)) => "worker_stopped",
                };
                schedule.ended(&key, reason, now);
            }
            if stop.cancelled() {
                break;
            }
            for key in schedule.starts(now) {
                if stop.cancelled() {
                    break;
                }
                let volume = &schedule.entries[&key].volume;
                let config = Config {
                    mode: Mode::Volume(volume.guid.clone()),
                    database: layout.data.join("volumes").join(&key).join("index.db"),
                    pipe: format!("volume-{key}"),
                    max_resolutions: 1_000_000,
                };
                let owner = owner_sid.clone();
                let worker_stop = Cancellation::default();
                let signal = worker_stop.clone();
                let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                let thread = std::thread::spawn(move || {
                    super::super::run_internal(config, signal, Some(owner), Some(ready_tx))
                });
                workers.insert(
                    key.clone(),
                    Worker {
                        ready: ready_rx,
                        thread,
                        stop: worker_stop,
                    },
                );
                schedule.started(&key);
            }
            *catalog.lock().unwrap() = schedule.snapshot();
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    })();
    stop.cancel();
    schedule.shutdown();
    for worker in workers.values() {
        worker.stop.cancel();
    }
    let mut checkpoint = 1u32;
    while !listener.is_finished() || workers.values().any(|w| !w.thread.is_finished()) {
        let _ = report(SERVICE_STOP_PENDING, checkpoint, 0);
        checkpoint = checkpoint.saturating_add(1);
        std::thread::sleep(Duration::from_millis(500));
    }
    let listener_result = listener
        .join()
        .map_err(|_| anyhow::anyhow!("service control thread panicked"));
    for (_, worker) in workers {
        let _ = worker.thread.join();
    }
    outcome?;
    listener_result??;
    Ok(())
}
fn volume_error(error: &anyhow::Error) -> &'static str {
    if error.to_string() == "discovered volume identity changed" {
        return "identity_changed";
    }
    if error
        .chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| matches!(e.raw_os_error(), Some(21 | 55 | 1167)))
    {
        return "volume_unavailable";
    }
    if error
        .chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| matches!(e.raw_os_error(), Some(1178 | 1179)))
    {
        "journal_unavailable"
    } else if error.to_string().contains("rebuild_required") {
        "rebuild_required"
    } else {
        "worker_unavailable"
    }
}
#[cfg(test)]
fn mark_failed(volume: &mut super::Volume, reason: &str) {
    volume.worker_started = false;
    volume.worker_error = Some(reason.into());
    volume.pipe = None;
}
#[cfg(test)]
fn observe_worker(
    worker: &mut Option<std::thread::JoinHandle<Result<super::super::protocol::PublicStatus>>>,
    volume: &mut super::Volume,
) {
    if worker.as_ref().is_some_and(|thread| thread.is_finished()) {
        let reason = match worker.take().unwrap().join() {
            Ok(Err(error)) => volume_error(&error),
            Err(_) => "worker_panicked",
            Ok(Ok(_)) => "worker_stopped",
        };
        mark_failed(volume, reason);
    }
}
unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let handle = unsafe {
        RegisterServiceCtrlHandlerExW(
            security::wide(SERVICE_NAME).as_ptr(),
            Some(control),
            ptr::null(),
        )
    };
    if handle.is_null() {
        return;
    }
    STATUS.store(handle, Ordering::Release);
    let stop = Cancellation::default();
    let _ = STOP.set(stop.clone());
    let outcome = std::panic::catch_unwind(|| {
        report(SERVICE_START_PENDING, 1, 0)?;
        work(stop.clone())
    });
    stop.cancel();
    let failed = !matches!(outcome, Ok(Ok(())));
    let _ = report(
        SERVICE_STOPPED,
        0,
        if failed {
            ERROR_SERVICE_SPECIFIC_ERROR
        } else {
            0
        },
    );
}
pub fn run() -> Result<()> {
    let mut name = security::wide(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    ensure!(
        unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } != 0,
        "service dispatcher: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_or_panicked_volume_does_not_cancel_healthy_volume_or_expose_error_text() {
        let stop = Cancellation::default();
        let mut first = super::super::Volume {
            guid: "fixture".into(),
            serial: Some(1),
            mounts: vec![],
            supported: true,
            reason: None,
            pipe: Some("fixture-pipe".into()),
            worker_started: true,
            worker_error: None,
        };
        let second = first.clone();
        let mut worker = Some(std::thread::spawn(|| {
            Err(anyhow::anyhow!("private candidate name must not escape"))
        }));
        while !worker.as_ref().unwrap().is_finished() {
            std::thread::yield_now();
        }
        observe_worker(&mut worker, &mut first);
        assert!(worker.is_none());
        assert!(!first.worker_started);
        assert!(first.pipe.is_none());
        assert_eq!(first.worker_error.as_deref(), Some("worker_unavailable"));
        assert!(second.worker_started);
        assert!(second.pipe.is_some());
        assert!(!stop.cancelled());
        let mut worker = Some(std::thread::spawn(|| panic!("synthetic volume panic")));
        while !worker.as_ref().unwrap().is_finished() {
            std::thread::yield_now();
        }
        observe_worker(&mut worker, &mut first);
        assert_eq!(first.worker_error.as_deref(), Some("worker_panicked"));
        assert!(worker.is_none());
        assert_eq!(
            volume_error(&std::io::Error::from_raw_os_error(1179).into()),
            "journal_unavailable"
        );
    }
}
