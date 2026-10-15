//! Fixed user-triggered elevation; never called by search or status polling.
use anyhow::{Result, ensure};
use std::{
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::FromRawHandle},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::FILE_SHARE_READ,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
};
fn arguments(action: &str, sha: &str, delete_index: bool) -> Result<String> {
    ensure!(
        sha.len() == 64
            && sha
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_lowercase()),
        "invalid frozen payload digest"
    );
    ensure!(
        matches!(
            action,
            "install"
                | "upgrade"
                | "repair"
                | "rollback"
                | "service-start"
                | "service-stop"
                | "uninstall"
        ),
        "unsupported service action"
    );
    ensure!(
        !delete_index || action == "uninstall",
        "delete-index only valid for uninstall"
    );
    Ok(if matches!(action, "install" | "upgrade" | "repair") {
        format!("{action} --expected-sha256 {sha}")
    } else if delete_index {
        "uninstall --delete-index".into()
    } else {
        action.into()
    })
}
pub fn launch(
    payload: &[u8],
    sha: &str,
    action: &str,
    delete_index: bool,
) -> Result<serde_json::Value> {
    let parameters = arguments(action, sha, delete_index)?;
    ensure!(
        crate::file_index::worker::service::payload_sha256(payload)? == sha,
        "bundled payload digest mismatch"
    );
    let folder = tempfile::Builder::new()
        .prefix("dao-index-install-")
        .tempdir()?;
    let _parents = crate::platform::pin_directory(folder.path())?;
    let image = folder.path().join("dao-index-worker.exe");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&image)?;
    file.write_all(payload)?;
    file.sync_all()?;
    drop(file);
    // Hold source bytes and all ancestors across RunAs so the elevated command
    // cannot be redirected by replacing the extracted path while UAC is pending.
    let mut _source = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&image)?;
    let mut frozen = Vec::new();
    Read::by_ref(&mut _source)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut frozen)?;
    ensure!(
        crate::file_index::worker::service::payload_sha256(&frozen)? == sha,
        "extracted payload changed before elevation"
    );
    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
    let verb = wide(std::ffi::OsStr::new("runas"));
    let image = wide(image.as_os_str());
    let args = wide(std::ffi::OsStr::new(&parameters));
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb.as_ptr(),
        lpFile: image.as_ptr(),
        lpParameters: args.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    if unsafe { ShellExecuteExW(&mut execute) } == 0 {
        let code = unsafe { GetLastError() };
        if code == ERROR_CANCELLED {
            return Ok(
                serde_json::json!({"state":"uac_cancelled","message":"已取消管理员授权，没有继续服务管理操作。"}),
            );
        }
        anyhow::bail!("无法启动固定服务管理程序（Windows 错误 {code}）");
    }
    ensure!(!execute.hProcess.is_null(), "管理进程句柄不可用");
    let process =
        unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(execute.hProcess.cast()) };
    use std::os::windows::io::AsRawHandle;
    // The desktop command runs on a blocking thread; no UI/session lock is held.
    let result = unsafe { WaitForSingleObject(process.as_raw_handle().cast(), 300_000) };
    if result == WAIT_TIMEOUT {
        // A privileged process is never killed by the ordinary client. Keep the
        // pinned source alive until it exits, while returning an explicit unknown result.
        std::thread::spawn(move || {
            unsafe { WaitForSingleObject(process.as_raw_handle().cast(), INFINITE) };
            drop(_source);
            drop(_parents);
            drop(folder);
        });
        return Ok(
            serde_json::json!({"state":"outcome_unknown","message":"管理程序尚未退出，请查看服务状态，不要重复启动同一操作。"}),
        );
    }
    ensure!(result == WAIT_OBJECT_0, "等待管理程序失败");
    let mut code = 0;
    ensure!(
        unsafe { GetExitCodeProcess(process.as_raw_handle().cast(), &mut code) } != 0,
        "无法读取管理结果"
    );
    Ok(serde_json::json!({"state":if code==0{"completed"}else{"failed"},"exit_code":code}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_arguments_reject_paths_and_commands() {
        let sha = "A".repeat(64);
        assert_eq!(
            arguments("install", &sha, false).unwrap(),
            format!("install --expected-sha256 {sha}")
        );
        assert!(arguments("install --report C:\\escape", &sha, false).is_err());
        assert!(arguments("service-stop", &sha, true).is_err());
        assert!(arguments("repair", "wrong", false).is_err());
    }
}
