//! Fixed shell handoffs in a short-lived process with a rebuilt user environment.
//! This is an internal transport, not a model tool or a command interpreter.
use crate::core::{Cancellation, FileIdentity};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MODE: &str = "--dao-shell-handoff-v1";
const MAX_REQUEST: usize = 64 * 1024;
const ACCEPTED: i32 = 0;
const CANCELLED: i32 = 20;
const REJECTED: i32 = 21;
const NO_ASSOCIATION: i32 = 22;
const INVALID: i32 = 23;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Document,
    Run,
    Admin,
    Location,
    ApplicationsSettings,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: Action,
    path: Option<PathBuf>,
    identity: Option<FileIdentity>,
}
impl Request {
    fn validate(&self) -> Result<()> {
        if matches!(self.action, Action::ApplicationsSettings) {
            ensure!(
                self.path.is_none() && self.identity.is_none(),
                "invalid_handoff"
            );
        } else {
            let path = self
                .path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("invalid_handoff"))?;
            ensure!(
                path.is_absolute() && self.identity.is_some(),
                "invalid_handoff"
            );
            let text = path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("invalid_handoff"))?;
            ensure!(
                !text.contains('\0') && text.len() <= 32768,
                "invalid_handoff"
            );
            if matches!(self.action, Action::Run | Action::Admin) {
                ensure!(
                    path.extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("exe")),
                    "invalid_handoff"
                );
            }
        }
        Ok(())
    }
}

/// Call before loading configuration, installing signal handlers or starting the UI.
pub fn run_internal_mode() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(MODE)) {
        return None;
    }
    if args.next().is_some() {
        return Some(INVALID);
    }
    Some(run_input())
}

fn run_input() -> i32 {
    use std::io::Read;
    let result = (|| -> Result<i32> {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take((MAX_REQUEST + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= MAX_REQUEST, "invalid_handoff");
        let request: Request = serde_json::from_slice(&bytes)?;
        request.validate()?;
        native(&request)
    })();
    result.unwrap_or(INVALID)
}

pub(crate) fn document(
    path: &Path,
    identity: &FileIdentity,
    excluded: &[String],
    cancel: &Cancellation,
) -> Result<bool> {
    execute(
        Request {
            action: Action::Document,
            path: Some(path.into()),
            identity: Some(identity.clone()),
        },
        excluded,
        cancel,
    )
}

/// Legacy overview actions have a closed vocabulary and never receive arguments or URLs.
pub fn application(
    action: &str,
    path: Option<&Path>,
    excluded: &[String],
    cancel: &Cancellation,
) -> Result<bool> {
    let action = match action {
        "run" => Action::Run,
        "admin" => Action::Admin,
        "location" => Action::Location,
        "uninstall" => Action::ApplicationsSettings,
        _ => anyhow::bail!("unsupported_handoff"),
    };
    let identity = path.map(crate::platform::identity).transpose()?;
    execute(
        Request {
            action,
            path: path.map(Path::to_path_buf),
            identity,
        },
        excluded,
        cancel,
    )
}

#[cfg(windows)]
fn execute(request: Request, excluded: &[String], cancel: &Cancellation) -> Result<bool> {
    use std::{
        io::Write,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    request.validate()?;
    let bytes = serde_json::to_vec(&request)?;
    ensure!(bytes.len() <= MAX_REQUEST, "invalid_handoff");
    let environment = crate::application_actions::native::clean_environment(excluded)?;
    let executable = std::env::current_exe()?;
    let expected = crate::platform::identity(&executable)?;
    let _held = crate::application_actions::launch::pin(&executable, &expected)
        .map_err(anyhow::Error::msg)?;
    cancel.check()?;
    let mut child = Command::new(executable)
        .arg(MODE)
        .env_clear()
        .envs(environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| anyhow::anyhow!("无法启动打开助手；未派发打开请求"))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("handoff_input_unavailable"))?;
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let began = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let written = writer.join().ok().is_some_and(|r| r.is_ok());
                ensure!(written, "打开结果未知；请先检查目标应用，不会自动重试");
                return match status.code() {
                    Some(ACCEPTED) => Ok(true),
                    Some(CANCELLED) => Ok(false),
                    Some(NO_ASSOCIATION) => anyhow::bail!(
                        "此类型尚未配置 Windows 默认打开应用，请先在系统设置中选择关联应用"
                    ),
                    Some(INVALID) => anyhow::bail!("打开助手拒绝请求或目标身份已变化；请重新查找"),
                    Some(REJECTED) => anyhow::bail!("Windows 未接受打开请求；未报告应用已启动"),
                    _ => anyhow::bail!("打开结果未知；请先检查目标应用，不会自动重试"),
                };
            }
            Ok(None) if !cancel.is_cancelled() && began.elapsed() < Duration::from_secs(90) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                anyhow::bail!("打开结果未知；请先检查目标应用，不会自动重试");
            }
        }
    }
}

#[cfg(not(windows))]
fn execute(_: Request, _: &[String], _: &Cancellation) -> Result<bool> {
    anyhow::bail!("此操作仅支持 Windows")
}
#[cfg(not(windows))]
fn native(_: &Request) -> Result<i32> {
    Ok(REJECTED)
}

#[cfg(windows)]
fn native(request: &Request) -> Result<i32> {
    use windows::{
        Win32::{
            Foundation::ERROR_CANCELLED,
            System::Com::{
                COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            },
            UI::{
                Shell::{
                    SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, SHOpenFolderAndSelectItems,
                    SHParseDisplayName, ShellExecuteExW,
                },
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
        core::PCWSTR,
    };
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _apartment = Apartment;
    if matches!(request.action, Action::Document) {
        return Ok(
            match crate::platform::open_document_direct(
                request.path.as_deref().unwrap(),
                request.identity.as_ref().unwrap(),
            ) {
                Ok(()) => ACCEPTED,
                Err(e) if e.to_string().contains("尚未配置 Windows 默认打开应用") => {
                    NO_ASSOCIATION
                }
                Err(_) => INVALID,
            },
        );
    }
    let _held = request
        .path
        .as_deref()
        .map(|p| {
            crate::application_actions::launch::pin(p, request.identity.as_ref().unwrap())
                .map_err(anyhow::Error::msg)
        })
        .transpose()?;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let target_text = request
        .path
        .as_deref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ms-settings:appsfeatures".into());
    let target = wide(target_text.strip_prefix(r"\\?\").unwrap_or(&target_text));
    if matches!(request.action, Action::Location) {
        let mut pidl = std::ptr::null_mut();
        unsafe {
            SHParseDisplayName(PCWSTR(target.as_ptr()), None, &mut pidl, 0, None)?;
        }
        let result = unsafe { SHOpenFolderAndSelectItems(pidl, None, 0) };
        unsafe {
            CoTaskMemFree(Some(pidl.cast()));
        }
        return Ok(if result.is_ok() { ACCEPTED } else { REJECTED });
    }
    let verb = wide(if matches!(request.action, Action::Admin) {
        "runas"
    } else {
        "open"
    });
    let directory = request
        .path
        .as_deref()
        .and_then(Path::parent)
        .map(|p| wide(&p.to_string_lossy()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(target.as_ptr()),
        lpDirectory: directory
            .as_ref()
            .map_or(PCWSTR::null(), |s| PCWSTR(s.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    Ok(match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => ACCEPTED,
        Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) => CANCELLED,
        Err(_) => REJECTED,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn internal_protocol_cannot_supply_arguments_urls_or_arbitrary_actions() {
        for value in [
            serde_json::json!({"action":"command","path":null,"identity":null}),
            serde_json::json!({"action":"applications_settings","path":null,"identity":null,"arguments":"anything"}),
        ] {
            assert!(serde_json::from_value::<Request>(value).is_err());
        }
        let bad = Request {
            action: Action::ApplicationsSettings,
            path: Some(PathBuf::from("https://example.invalid")),
            identity: None,
        };
        assert!(bad.validate().is_err());
    }
}
