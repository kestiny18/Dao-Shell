//! Fixed provider protocol. All COM activation/catalog calls live outside the request host.
use super::contract::*;
use crate::core::Cancellation;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
#[cfg(any(windows, test))]
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MODE: &str = "--dao-software-provider-v1";
const MAX_REQUEST: usize = 64 * 1024;
const MAX_RESPONSE: usize = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Search {
        query: String,
    },
    Inspect {
        identity: Identity,
    },
    Compare {
        identity: Identity,
        installed: String,
    },
    Recover {
        identity: Identity,
    },
    Dispatch {
        plan: Box<Plan>,
    },
}
pub struct IsolatedProvider {
    cancel: Cancellation,
    excluded: Vec<String>,
}
impl IsolatedProvider {
    pub fn new(cancel: Cancellation, excluded: Vec<String>) -> Self {
        Self { cancel, excluded }
    }
    #[cfg(windows)]
    fn request(&self, request: Request) -> Result<Value> {
        use std::os::windows::process::CommandExt;
        self.cancel.check()?;
        let executable = std::env::current_exe()?;
        let identity = crate::platform::identity(&executable)?;
        let _held = crate::application_actions::launch::pin(&executable, &identity)
            .map_err(anyhow::Error::msg)?;
        let mut command = Command::new(executable);
        command
            .arg(MODE)
            .env_clear()
            .envs(crate::application_actions::native::clean_environment(
                &self.excluded,
            )?)
            .creation_flags(0x08000000);
        let timeout = Duration::from_secs(if matches!(request, Request::Dispatch { .. }) {
            900
        } else {
            30
        });
        let bytes = serde_json::to_vec(&request)?;
        let bytes = exchange(command, bytes, &self.cancel, timeout)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("provider_invalid_response"))?;
        ensure!(value.get("error").is_none(), "provider_query_failed");
        value
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("provider_invalid_response"))
    }
    #[cfg(not(windows))]
    fn request(&self, _: Request) -> Result<Value> {
        let _ = (&self.cancel, &self.excluded);
        anyhow::bail!("unsupported_platform")
    }
}
impl Provider for IsolatedProvider {
    fn search(&mut self, query: &str) -> Result<Vec<Package>> {
        Ok(serde_json::from_value(self.request(Request::Search {
            query: query.into(),
        })?)?)
    }
    fn inspect(&mut self, identity: &Identity) -> Result<Package> {
        Ok(serde_json::from_value(self.request(Request::Inspect {
            identity: identity.clone(),
        })?)?)
    }
    fn recover(&mut self, identity: &Identity) -> Result<Recovery> {
        Ok(serde_json::from_value(self.request(Request::Recover {
            identity: identity.clone(),
        })?)?)
    }
    fn compare_versions(&self, identity: &Identity, installed: &str) -> Result<std::cmp::Ordering> {
        let result = self.request(Request::Compare {
            identity: identity.clone(),
            installed: installed.into(),
        })?;
        match result.as_i64() {
            Some(-1) => Ok(std::cmp::Ordering::Less),
            Some(0) => Ok(std::cmp::Ordering::Equal),
            Some(1) => Ok(std::cmp::Ordering::Greater),
            _ => anyhow::bail!("provider_invalid_response"),
        }
    }
    fn dispatch(
        &mut self,
        plan: &Plan,
        _: &Cancellation,
        progress: &mut dyn FnMut(&str),
    ) -> Result<DispatchResult> {
        progress("provider_dispatch_pending");
        // Engine has already durably recorded dispatching. Any lost worker result
        // remains unknown; killing this worker does not stop the external installer.
        Ok(serde_json::from_value(self.request(
            Request::Dispatch {
                plan: Box::new(plan.clone()),
            },
        )?)?)
    }
}

#[cfg(any(windows, test))]
fn exchange(
    mut command: Command,
    input: Vec<u8>,
    cancel: &Cancellation,
    timeout: Duration,
) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    ensure!(input.len() <= MAX_REQUEST, "provider_request_budget");
    cancel.check()?;
    // Files, not pipes: a descendant retaining a stdio handle must never leave
    // a blocked reader/writer thread after the worker exits or is cancelled.
    let mut request = tempfile::tempfile()?;
    request.write_all(&input)?;
    request.seek(SeekFrom::Start(0))?;
    let mut response = tempfile::tempfile()?;
    let mut child = command
        .stdin(Stdio::from(request))
        .stdout(Stdio::from(response.try_clone()?))
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    let result = loop {
        match response.metadata() {
            Ok(m) if m.len() > MAX_RESPONSE as u64 => {
                break Err(anyhow::anyhow!("provider_response_budget"));
            }
            Err(e) => break Err(e.into()),
            _ => (),
        }
        if cancel.is_cancelled() || start.elapsed() >= timeout {
            break Err(anyhow::anyhow!("provider_cancelled_or_timeout"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("provider_worker_failed"))
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error.into()),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result?;
    response.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    response
        .take((MAX_RESPONSE + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_RESPONSE, "provider_response_budget");
    Ok(bytes)
}

pub fn run_internal_mode() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(MODE)) {
        return None;
    }
    if args.next().is_some() {
        return Some(23);
    }
    Some(run_input().map_or(23, |_| 0))
}
fn run_input() -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_REQUEST, "provider_request_budget");
    let request: Request = serde_json::from_slice(&bytes)?;
    let result = native(request);
    let value = match result {
        Ok(value) => serde_json::json!({"result":value}),
        Err(_) => serde_json::json!({"error":"provider_query_failed"}),
    };
    let bytes = serde_json::to_vec(&value)?;
    ensure!(bytes.len() <= MAX_RESPONSE, "provider_response_budget");
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
#[cfg(windows)]
fn native(request: Request) -> Result<Value> {
    let mut provider = super::typed::NativeProvider::open()?;
    match request {
        Request::Search { query } => Ok(serde_json::to_value(provider.search(&query)?)?),
        Request::Inspect { identity } => Ok(serde_json::to_value(provider.inspect(&identity)?)?),
        Request::Recover { identity } => Ok(serde_json::to_value(provider.recover(&identity)?)?),
        Request::Compare {
            identity,
            installed,
        } => Ok(serde_json::json!(
            match provider.compare_versions(&identity, &installed)? {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        )),
        Request::Dispatch { plan } => {
            // Revalidate inside the isolated process too; helper execution never
            // relaxes the parent-confirmed exact target or accepted agreements.
            let current = provider.inspect(&plan.package.identity)?;
            plan.revalidate(&current, plan.package.agreement_digest.as_deref())?;
            Ok(serde_json::to_value(provider.dispatch(
                &plan,
                &Cancellation::default(),
                &mut |_| {},
            )?)?)
        }
    }
}
#[cfg(not(windows))]
fn native(_: Request) -> Result<Value> {
    anyhow::bail!("unsupported_platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(mode: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args([
            "--exact",
            "software::isolated::tests::provider_process_fixture",
            "--nocapture",
        ])
        .env("DAO_PROVIDER_PROCESS_FIXTURE", mode);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        c
    }
    #[test]
    fn provider_process_fixture() {
        let Ok(mode) = std::env::var("DAO_PROVIDER_PROCESS_FIXTURE") else {
            return;
        };
        match mode.as_str() {
            "hold" => {
                let root = std::path::PathBuf::from(
                    std::env::var_os("DAO_PROVIDER_HOLD_FIXTURE").unwrap(),
                );
                std::fs::write(root.join("ready"), b"ready").unwrap();
                let start = Instant::now();
                while !root.join("release").exists() && start.elapsed() < Duration::from_secs(15) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                std::fs::write(root.join("done"), b"done").unwrap();
            }
            "sleep" => std::thread::sleep(Duration::from_secs(30)),
            "flood" => {
                std::io::stdout()
                    .write_all(&vec![b'x'; MAX_RESPONSE + 100])
                    .unwrap();
                std::thread::sleep(Duration::from_secs(10));
            }
            "descendant" => {
                // Deliberate orphan fixture: the supervising test releases it
                // after proving the worker can exit while stdout remains held.
                #[allow(clippy::zombie_processes)]
                let _child = fixture("hold")
                    .stdout(Stdio::inherit())
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();
                let root = std::path::PathBuf::from(
                    std::env::var_os("DAO_PROVIDER_HOLD_FIXTURE").unwrap(),
                );
                let start = Instant::now();
                while !root.join("ready").exists() && start.elapsed() < Duration::from_secs(5) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                assert!(root.join("ready").exists());
                println!("synthetic response");
            }
            _ => panic!("invalid fixture"),
        }
        std::process::exit(0);
    }
    #[test]
    fn blocked_read_and_cancel_release_parent_and_enforce_output_bound() {
        let now = Instant::now();
        assert!(
            exchange(
                fixture("sleep"),
                vec![],
                &Cancellation::default(),
                Duration::from_millis(200)
            )
            .unwrap_err()
            .to_string()
            .contains("timeout")
        );
        // Include OS process startup under the full parallel suite, but remain
        // far below the fixture's 30-second blocked operation.
        assert!(now.elapsed() < Duration::from_secs(15));
        let cancel = Cancellation::default();
        let trigger = cancel.clone();
        let task = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            trigger.cancel();
        });
        assert!(exchange(fixture("sleep"), vec![], &cancel, Duration::from_secs(5)).is_err());
        task.join().unwrap();
        assert!(
            exchange(
                fixture("flood"),
                vec![],
                &Cancellation::default(),
                Duration::from_secs(5)
            )
            .unwrap_err()
            .to_string()
            .contains("budget")
        );
    }
    #[test]
    fn inherited_stdout_cannot_keep_request_waiting_after_worker_exit() {
        let directory = tempfile::tempdir().unwrap();
        let mut command = fixture("descendant");
        command.env("DAO_PROVIDER_HOLD_FIXTURE", directory.path());
        let result = exchange(
            command,
            vec![],
            &Cancellation::default(),
            Duration::from_secs(8),
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&result).contains("synthetic response"));
        assert!(directory.path().join("ready").exists());
        assert!(
            !directory.path().join("done").exists(),
            "must return while descendant still holds stdout"
        );
        std::fs::write(directory.path().join("release"), b"release").unwrap();
        let start = Instant::now();
        while !directory.path().join("done").exists() && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(directory.path().join("done").exists());
    }
    #[test]
    fn protocol_rejects_arbitrary_actions_and_extra_fields() {
        for value in [
            serde_json::json!({"kind":"command","text":"cmd"}),
            serde_json::json!({"kind":"search","query":"fixture","command":"cmd"}),
        ] {
            assert!(serde_json::from_value::<Request>(value).is_err());
        }
    }
}
