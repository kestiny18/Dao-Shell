use super::*;
use std::ptr;
use windows_sys::Win32::{
    Foundation::*,
    Security::Authorization::ConvertSidToStringSidW,
    Security::*,
    System::{
        RemoteDesktop::ProcessIdToSessionId, SystemInformation::GetSystemWindowsDirectoryW,
        Threading::*,
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn open(pid: u32, rights: u32) -> Result<Handle, &'static str> {
    let h = unsafe { OpenProcess(rights, 0, pid) };
    if h.is_null() {
        Err(if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
            "already_exited"
        } else {
            "access_denied"
        })
    } else {
        Ok(Handle(h))
    }
}
fn created(h: HANDLE) -> Option<String> {
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return None;
    }
    let value = (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime);
    (value > 0).then(|| value.to_string())
}
pub fn creation(pid: u32) -> Option<String> {
    created(open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?.0)
}
fn sid(h: HANDLE) -> Result<String, &'static str> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(h, TOKEN_QUERY, &mut raw) } == 0 {
        return Err("identity_unavailable");
    }
    let token = Handle(raw);
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut needed);
    }
    if needed == 0 || needed > 65536 {
        return Err("identity_unavailable");
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err("identity_unavailable");
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err("identity_unavailable");
    }
    let mut len = 0;
    while unsafe { *text.add(len) } != 0 && len < 256 {
        len += 1;
    }
    let result = String::from_utf16(unsafe { std::slice::from_raw_parts(text, len) })
        .map_err(|_| "identity_unavailable");
    unsafe {
        LocalFree(text.cast());
    }
    result
}
fn session(pid: u32) -> Result<u32, &'static str> {
    let mut value = 0;
    if unsafe { ProcessIdToSessionId(pid, &mut value) } == 0 {
        Err("identity_unavailable")
    } else {
        Ok(value)
    }
}
fn protected(image: &str, own_image: &str, windows: &str) -> bool {
    let image = image.to_lowercase();
    image == own_image.to_lowercase()
        || image.starts_with(&format!(
            "{}\\",
            windows.trim_end_matches('\\').to_lowercase()
        ))
        || ["msedgewebview2.exe", "dao-index-worker.exe", "explorer.exe"]
            .contains(&image.rsplit('\\').next().unwrap_or(""))
}
fn caller_matches(
    owner: &str,
    current_owner: &str,
    session: u32,
    current_session: u32,
) -> Result<(), &'static str> {
    if owner != current_owner {
        return Err("other_user");
    }
    if session == 0 || session != current_session {
        return Err("other_session");
    }
    Ok(())
}
fn identity(h: HANDLE, pid: u32) -> Result<Identity, &'static str> {
    if pid <= 4 || pid == std::process::id() {
        return Err("protected_process");
    }
    let created = created(h).ok_or("identity_unavailable")?;
    let mut critical = 0;
    if unsafe { IsProcessCritical(h, &mut critical) } == 0 {
        return Err("identity_unavailable");
    }
    if critical != 0 {
        return Err("protected_process");
    }
    let owner = sid(h)?;
    let session = session(pid)?;
    caller_matches(
        &owner,
        &sid(unsafe { GetCurrentProcess() })?,
        session,
        self::session(std::process::id())?,
    )?;
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    if unsafe { QueryFullProcessImageNameW(h, 0, image.as_mut_ptr(), &mut length) } == 0 {
        return Err("identity_unavailable");
    }
    let image =
        String::from_utf16(&image[..length as usize]).map_err(|_| "identity_unavailable")?;
    let own = std::env::current_exe().map_err(|_| "identity_unavailable")?;
    let mut windows = vec![0u16; 32768];
    let n =
        unsafe { GetSystemWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) } as usize;
    if n == 0 || n >= windows.len() {
        return Err("identity_unavailable");
    }
    let windows = String::from_utf16(&windows[..n]).map_err(|_| "identity_unavailable")?;
    if protected(&image, &own.to_string_lossy(), &windows) {
        return Err("protected_process");
    }
    Ok(Identity {
        pid,
        created,
        image,
        owner,
        session,
    })
}
pub fn capture(pid: u32) -> Result<Identity, &'static str> {
    identity(open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?.0, pid)
}
pub fn enumerate() -> Vec<Identity> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system
        .processes()
        .keys()
        .take(4096)
        .filter_map(|p| capture(p.as_u32()).ok())
        .collect()
}
pub(super) fn terminate(expected: &Identity, cancel: &Cancellation) -> &'static str {
    let h = match open(
        expected.pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
    ) {
        Ok(h) => h,
        Err(code) => return code,
    };
    if unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
        return "already_exited";
    }
    match identity(h.0, expected.pid) {
        Ok(actual) if actual == *expected => {}
        Ok(_) => return "identity_changed",
        Err(code) => return code,
    }
    if cancel.is_cancelled() {
        return "cancelled";
    }
    if unsafe { TerminateProcess(h.0, 1) } == 0 {
        return if unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
            "already_exited"
        } else {
            "access_denied"
        };
    }
    match unsafe { WaitForSingleObject(h.0, 50) } {
        WAIT_OBJECT_0 => "terminated",
        _ => "pending",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_identity_rejects_self_and_system_images() {
        assert!(capture(std::process::id()).is_err());
        assert!(capture(4).is_err());
        assert!(protected(
            r"C:\Windows\System32\fixture.exe",
            r"C:\app.exe",
            r"C:\Windows"
        ));
        assert!(protected(r"C:\app.exe", r"C:\app.exe", r"C:\Windows"));
        assert!(creation(std::process::id()).is_some());
        assert_eq!(caller_matches("other", "owner", 1, 1), Err("other_user"));
        assert_eq!(caller_matches("owner", "owner", 0, 1), Err("other_session"));
        assert_eq!(caller_matches("owner", "owner", 2, 1), Err("other_session"));
        assert!(caller_matches("owner", "owner", 1, 1).is_ok());
    }
    #[test]
    #[ignore = "owned fixture helper, invoked by termination test"]
    fn owned_process_helper() {
        if let Some(path) = std::env::var_os("DAO_P12_OWNED_PROCESS_READY") {
            std::fs::write(path, b"ready").unwrap();
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }
    #[test]
    fn only_owned_fixture_can_terminate_after_exact_native_identity_and_cancel_recheck() {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("owned-fixture.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let ready = dir.path().join("ready");
        let mut child = Child(
            std::process::Command::new(exe)
                .args([
                    "--ignored",
                    "--exact",
                    "process_control::native::tests::owned_process_helper",
                ])
                .env("DAO_P12_OWNED_PROCESS_READY", &ready)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !ready.exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let observed = capture(child.0.id()).unwrap();
        assert!(observed.created.parse::<u64>().unwrap() > 9_007_199_254_740_992);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert_eq!(super::super::terminate(&observed, &cancel), "cancelled");
        assert!(child.0.try_wait().unwrap().is_none());
        let cancel = Cancellation::default();
        let mut reused = observed.clone();
        reused.created = (observed.created.parse::<u64>().unwrap() + 1).to_string();
        assert_eq!(
            super::super::terminate(&reused, &cancel),
            "identity_changed"
        );
        reused = observed.clone();
        reused.image.push_str(".changed");
        assert_eq!(
            super::super::terminate(&reused, &cancel),
            "identity_changed"
        );
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(matches!(
            super::super::terminate(&observed, &cancel),
            "terminated" | "pending"
        ));
        assert!(child.0.wait().is_ok());
        assert_eq!(
            super::super::terminate(&observed, &cancel),
            "already_exited"
        );
    }
}
