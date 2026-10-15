use super::super::{security, transport};
use super::{
    SERVICE_NAME,
    package::{Layout, Manifest},
    scm::{Controller, Native},
};
use crate::file_index::Cancellation;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_sys::Win32::Foundation::HANDLE;
pub(super) const PIPE: &str = "service-control";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Health {
    pub version: u32,
    pub service: String,
    pub build: String,
    pub owner_sid: String,
    pub volumes: Vec<super::Volume>,
}
fn verify_evidence(
    before: &super::scm::ServiceInfo,
    after: &super::scm::ServiceInfo,
    first: super::peer::Peer,
    last: super::peer::Peer,
    command: &str,
) -> Result<()> {
    ensure!(
        before.state == 4
            && before.pid != 0
            && before.account.eq_ignore_ascii_case("LocalSystem")
            && before.image == command,
        "SCM service identity mismatch"
    );
    ensure!(
        first.pid == before.pid && first.session == 0 && last == first,
        "pipe peer differs from installed SYSTEM service"
    );
    ensure!(
        after.pid == before.pid
            && after.state == 4
            && after.image == before.image
            && after.account == before.account,
        "service changed during authentication"
    );
    Ok(())
}
pub(super) fn verify_server(pipe: HANDLE, layout: &Layout, manifest: &Manifest) -> Result<()> {
    let before = Native
        .inspect()?
        .ok_or_else(|| anyhow::anyhow!("service not installed"))?;
    let first = super::peer::identity(pipe)?;
    // The already-open kernel pipe's owner/DACL prevents an ordinary process
    // from manufacturing an instance even if a stale PID has been reused.
    super::peer::verify(pipe, &manifest.owner_sid)?;
    let _ancestors = layout.hold_ancestors()?;
    let _programs = super::super::storage::hold(&layout.programs, true)?;
    let image = layout.verify_image(manifest, &manifest.current)?;
    let _directory = super::super::storage::hold(image.parent().unwrap(), true)?;
    let _image = super::super::storage::hold(&image, false)?;
    let last = super::peer::identity(pipe)?;
    let after = Native
        .inspect()?
        .ok_or_else(|| anyhow::anyhow!("service exited"))?;
    verify_evidence(
        &before,
        &after,
        first,
        last,
        &layout.command(&manifest.current)?,
    )
}
pub(super) fn query(layout: &Layout, manifest: &Manifest) -> Result<Health> {
    let cancel = Cancellation::with_timeout(Duration::from_secs(3));
    query_controlled(layout, manifest, &cancel)
}
pub(super) fn query_controlled(
    layout: &Layout,
    manifest: &Manifest,
    cancel: &Cancellation,
) -> Result<Health> {
    let pipe = transport::connect_unverified(PIPE, cancel)?;
    verify_server(security::raw(&pipe), layout, manifest)?;
    transport::send(
        security::raw(&pipe),
        br#"{"version":1,"operation":"health"}"#,
        cancel,
    )?;
    let response: Health =
        serde_json::from_slice(&transport::receive(security::raw(&pipe), cancel)?)?;
    ensure!(
        response.version == 1
            && response.service == SERVICE_NAME
            && response.build == manifest.current
            && response.owner_sid == manifest.owner_sid,
        "service health identity mismatch"
    );
    transport::acknowledge(security::raw(&pipe), cancel)?;
    Ok(response)
}
pub(super) fn wait(layout: &Layout, manifest: &Manifest) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(health) = query(layout, manifest)
            && health
                .volumes
                .iter()
                .any(|volume| volume.worker_started && volume.worker_error.is_none())
        {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "authenticated service pipe or initialized volume health timed out"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub(super) fn serve(
    manifest: Manifest,
    volumes: Arc<Mutex<Vec<super::Volume>>>,
    stop: Cancellation,
    ready: std::sync::mpsc::Sender<Result<()>>,
) -> Result<()> {
    let mut owner = security::current()?;
    owner.sid = manifest.owner_sid.clone();
    let pipes = match transport::listeners_for(PIPE, &owner, false) {
        Ok(p) => p,
        Err(e) => {
            let _ = ready.send(Err(anyhow::anyhow!("service control pipe unavailable")));
            return Err(e);
        }
    };
    let mut threads = Vec::new();
    for pipe in pipes {
        let stop = stop.clone();
        let volumes = volumes.clone();
        let build = manifest.current.clone();
        let sid = manifest.owner_sid.clone();
        threads.push(std::thread::spawn(move || {
            while !stop.cancelled() {
                if let Err(error) = transport::accept(security::raw(&pipe), &stop) {
                    if stop.cancelled() {
                        break;
                    }
                    stop.cancel();
                    return Err(error);
                }
                let _ = (|| -> Result<()> {
                    let timeout = Cancellation::with_timeout(Duration::from_secs(3));
                    let bytes = transport::receive(security::raw(&pipe), &timeout)?;
                    security::service_caller(security::raw(&pipe), &sid)?;
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Request {
                        version: u32,
                        operation: String,
                    }
                    let request: Request = serde_json::from_slice(&bytes)?;
                    ensure!(
                        request.version == 1 && request.operation == "health",
                        "unsupported control request"
                    );
                    let response = serde_json::to_vec(&Health {
                        version: 1,
                        service: SERVICE_NAME.into(),
                        build: build.clone(),
                        owner_sid: sid.clone(),
                        volumes: volumes.lock().unwrap().clone(),
                    })?;
                    transport::send(security::raw(&pipe), &response, &timeout)?;
                    transport::wait_ack(security::raw(&pipe), &timeout)
                })();
                transport::disconnect(security::raw(&pipe));
            }
            Ok(())
        }));
    }
    let _ = ready.send(Ok(()));
    while !stop.cancelled() {
        if threads.iter().any(|thread| thread.is_finished()) {
            stop.cancel();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut result = Ok(());
    for thread in threads {
        match thread.join() {
            Ok(Ok(())) => (),
            Ok(Err(error)) => result = Err(error),
            Err(_) => result = Err(anyhow::anyhow!("control listener panicked")),
        }
    }
    result
}

#[cfg(test)]
mod authentication_tests {
    use super::*;
    #[test]
    fn kernel_peer_must_match_unchanged_fixed_scm_service() {
        let command = "\"C:\\protected\\digest\\worker.exe\" service-run";
        let info = super::super::scm::ServiceInfo {
            image: command.into(),
            account: "LocalSystem".into(),
            state: 4,
            pid: 123,
            start_type: 2,
        };
        let peer = super::super::peer::Peer {
            pid: 123,
            session: 0,
        };
        assert!(verify_evidence(&info, &info, peer, peer, command).is_ok());
        let mut bad = info.clone();
        bad.account = "LocalService".into();
        assert!(verify_evidence(&bad, &bad, peer, peer, command).is_err());
        bad = info.clone();
        bad.image = "\"C:\\user\\fake.exe\" service-run".into();
        assert!(verify_evidence(&bad, &bad, peer, peer, command).is_err());
        bad = info.clone();
        bad.pid += 1;
        assert!(verify_evidence(&info, &bad, peer, peer, command).is_err());
        bad = info.clone();
        bad.state = 1;
        assert!(verify_evidence(&info, &bad, peer, peer, command).is_err());
        let mut wrong = peer;
        wrong.session = 1;
        assert!(verify_evidence(&info, &info, wrong, wrong, command).is_err());
        wrong = peer;
        wrong.pid += 1;
        assert!(verify_evidence(&info, &info, peer, wrong, command).is_err());
    }
}
