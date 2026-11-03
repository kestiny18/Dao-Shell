//! Fixed native launch and read-only observation. Never invokes a shell or UAC.
use super::launch::Target;
use crate::{core::Cancellation, process_control::Identity};
use anyhow::{Result, ensure};
use serde_json::{Value, json};

pub fn elevated() -> bool {
    #[cfg(windows)]
    {
        windows::elevated_process(unsafe {
            windows_sys::Win32::System::Threading::GetCurrentProcess()
        })
        .unwrap_or(true)
    }
    #[cfg(not(windows))]
    {
        true
    }
}
pub fn launch(target: &Target, excluded: &[String], cancel: &Cancellation) -> Result<Value> {
    #[cfg(windows)]
    {
        windows::launch(target, excluded, cancel)
    }
    #[cfg(not(windows))]
    {
        let _ = (target, excluded, cancel);
        anyhow::bail!("unsupported_platform")
    }
}
pub fn reconcile(identity: &Identity) -> Value {
    match crate::process_control::capture(identity.pid) {
        Ok(actual) if actual == *identity => {
            json!({"process_state":"still_running","code":"original_instance_verified"})
        }
        // A missing PID does not prove what happened to the previous instance.
        _ => json!({"process_state":"unknown","code":"original_instance_unverifiable"}),
    }
}

/// Rebuild from the user token, never from the host's possibly secret-bearing environment.
#[cfg(windows)]
pub(crate) fn clean_environment(excluded: &[String]) -> Result<Vec<(String, String)>> {
    let block = windows::environment(excluded)?;
    let text = String::from_utf16(&block)?;
    Ok(text
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        collections::BTreeMap,
        ptr,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Security::*,
        System::{Environment::*, Threading::*},
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    pub(super) fn elevated_process(process: HANDLE) -> Option<bool> {
        let mut raw = ptr::null_mut();
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
            return None;
        }
        let token = Handle(raw);
        let mut info = TOKEN_ELEVATION::default();
        let mut size = 0;
        (unsafe {
            GetTokenInformation(
                token.0,
                TokenElevation,
                (&mut info as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of_val(&info) as u32,
                &mut size,
            )
        } != 0)
            .then_some(info.TokenIsElevated != 0)
    }
    pub(super) fn environment(excluded: &[String]) -> Result<Vec<u16>> {
        let mut raw = ptr::null_mut();
        ensure!(
            unsafe {
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut raw)
            } != 0,
            "environment_unavailable"
        );
        let token = Handle(raw);
        let mut block = ptr::null_mut();
        ensure!(
            unsafe { CreateEnvironmentBlock(&mut block, token.0, 0) } != 0,
            "environment_unavailable"
        );
        struct Block(*mut std::ffi::c_void);
        impl Drop for Block {
            fn drop(&mut self) {
                unsafe {
                    DestroyEnvironmentBlock(self.0);
                }
            }
        }
        let guard = Block(block);
        let mut position = 0usize;
        let mut values = BTreeMap::new();
        while position < 131072 {
            let start = position;
            while position < 131072 && unsafe { *guard.0.cast::<u16>().add(position) } != 0 {
                position += 1;
            }
            ensure!(position < 131072, "environment_unavailable");
            if start == position {
                break;
            }
            let text = String::from_utf16(unsafe {
                std::slice::from_raw_parts(guard.0.cast::<u16>().add(start), position - start)
            })?;
            if let Some((key, value)) = text.split_once('=') {
                let upper = key.to_uppercase();
                if !key.is_empty()
                    && !upper.starts_with("DAO_")
                    && !upper.starts_with("WEBVIEW2_")
                    && !upper.starts_with("TAURI_")
                    && !excluded.iter().any(|e| e.eq_ignore_ascii_case(key))
                {
                    values.insert(upper, (key.to_owned(), value.to_owned()));
                }
            }
            position += 1;
        }
        ensure!(values.contains_key("SYSTEMROOT"), "environment_unavailable");
        let mut out = Vec::new();
        for (_, (key, value)) in values {
            out.extend(format!("{key}={value}").encode_utf16());
            out.push(0);
        }
        out.push(0);
        Ok(out)
    }
    pub(super) fn launch(
        target: &Target,
        excluded: &[String],
        cancel: &Cancellation,
    ) -> Result<Value> {
        ensure!(!super::elevated(), "elevated_host");
        let _link = super::super::launch::pin(&target.link, &target.link_identity)
            .map_err(|_| anyhow::anyhow!("identity_changed"))?;
        let _exe = super::super::launch::pin(&target.exe, &target.identity)
            .map_err(|_| anyhow::anyhow!("identity_changed"))?;
        let text = target
            .exe
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("launch_unavailable"))?;
        ensure!(
            !text.contains('"') && !text.contains('\0'),
            "launch_unavailable"
        );
        let app = wide(text);
        let mut command = wide(&format!("\"{text}\""));
        let cwd = wide(
            target
                .exe
                .parent()
                .and_then(|p| p.to_str())
                .ok_or_else(|| anyhow::anyhow!("launch_unavailable"))?,
        );
        let env = environment(excluded)?;
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        cancel.check()?;
        if unsafe {
            CreateProcessW(
                app.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                CREATE_UNICODE_ENVIRONMENT,
                env.as_ptr().cast(),
                cwd.as_ptr(),
                &startup,
                &mut info,
            )
        } == 0
        {
            anyhow::bail!(if unsafe { GetLastError() } == ERROR_ELEVATION_REQUIRED {
                "elevation_required"
            } else {
                "launch_failed"
            });
        }
        let process = Handle(info.hProcess);
        let _thread = Handle(info.hThread);
        let native = crate::process_control::capture(info.dwProcessId).ok();
        let began = Instant::now();
        let mut exited = false;
        while began.elapsed() < Duration::from_secs(3) {
            if unsafe { WaitForSingleObject(process.0, 50) } == WAIT_OBJECT_0 {
                exited = true;
                break;
            }
            if cancel.is_cancelled() {
                break;
            }
        }
        let mut exit_code = 0;
        let code = if exited && unsafe { GetExitCodeProcess(process.0, &mut exit_code) } != 0 {
            Some(exit_code)
        } else {
            None
        };
        Ok(
            json!({"process_created":true,"process_state":if exited {"exited"}else if cancel.is_cancelled(){"unknown"}else{"still_running"},"exit_code":code,"process":native,"code":"process_created_not_window_ready","observation_cancelled":cancel.is_cancelled()}),
        )
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn child_environment_comes_from_user_environment_not_parent_injection() {
            let key = "DAO_PHASE2_SENTINEL_SECRET";
            // Isolated child-only environment construction; never prints source values.
            let block = environment(&[key.into(), "PATH".into()]).unwrap();
            let text = String::from_utf16_lossy(&block);
            assert!(!text.to_uppercase().contains("DAO_"));
            assert!(
                !text
                    .split('\0')
                    .any(|p| p.to_uppercase().starts_with("PATH="))
            );
            assert!(text.to_uppercase().contains("SYSTEMROOT="));
        }
        #[test]
        fn configured_credential_names_are_removed_from_rebuilt_persistent_environment() {
            // PATH is a nonsecret persistent user-environment slot. Naming it as the
            // configured credential exercises registry-derived filtering without
            // reading real keys or modifying the user's environment registry.
            let config = crate::config::Config {
                model: Some(crate::config::ModelConfig {
                    endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
                    model: "unused".into(),
                    api_key_env: "pAtH".into(),
                }),
                ..Default::default()
            };
            let original = super::super::clean_environment(&[]).unwrap();
            assert!(
                original
                    .iter()
                    .any(|(key, _)| key.eq_ignore_ascii_case("PATH"))
            );
            let clean =
                super::super::clean_environment(&config.credential_environment_names()).unwrap();
            assert!(
                !clean
                    .iter()
                    .any(|(key, _)| key.eq_ignore_ascii_case("PATH"))
            );
            assert!(
                clean
                    .iter()
                    .any(|(key, _)| key.eq_ignore_ascii_case("SystemRoot"))
            );
        }
    }
}
