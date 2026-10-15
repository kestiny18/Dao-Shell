use super::super::security::wide;
use super::SERVICE_NAME;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::{DELETE, WRITE_DAC},
    System::Services::*,
};

struct Handle(SC_HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}
fn manager(rights: u32) -> Result<Handle> {
    let h = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), rights) };
    ensure!(
        !h.is_null(),
        "SCM open: {}",
        std::io::Error::last_os_error()
    );
    Ok(Handle(h))
}
fn open(rights: u32) -> Result<Option<Handle>> {
    let manager = manager(SC_MANAGER_CONNECT)?;
    let h = unsafe { OpenServiceW(manager.0, wide(SERVICE_NAME).as_ptr(), rights) };
    if h.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
            return Ok(None);
        }
        return Err(error.into());
    }
    Ok(Some(Handle(h)))
}
unsafe fn string(value: *const u16) -> Result<String> {
    ensure!(!value.is_null(), "missing SCM string");
    let mut len = 0;
    while len < 32768 && unsafe { *value.add(len) } != 0 {
        len += 1;
    }
    ensure!(len < 32768, "SCM string exceeds bound");
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(value, len)
    })?)
}
#[derive(Clone, Debug, Serialize)]
pub(super) struct ServiceInfo {
    pub image: String,
    pub account: String,
    pub state: u32,
    pub pid: u32,
    pub start_type: u32,
}
fn inspect_handle(h: &Handle) -> Result<ServiceInfo> {
    let mut needed = 0;
    unsafe { QueryServiceConfigW(h.0, ptr::null_mut(), 0, &mut needed) };
    ensure!(
        (size_of::<QUERY_SERVICE_CONFIGW>() as u32..=65536).contains(&needed),
        "invalid SCM config size"
    );
    let mut buffer = vec![0u8; needed as usize];
    ensure!(
        unsafe { QueryServiceConfigW(h.0, buffer.as_mut_ptr().cast(), needed, &mut needed) } != 0,
        "SCM config query failed"
    );
    let config = unsafe { ptr::read_unaligned(buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>()) };
    ensure!(
        config.dwServiceType == SERVICE_WIN32_OWN_PROCESS,
        "unexpected service process type"
    );
    let mut state = SERVICE_STATUS_PROCESS::default();
    ensure!(
        unsafe {
            QueryServiceStatusEx(
                h.0,
                SC_STATUS_PROCESS_INFO,
                (&mut state as *mut SERVICE_STATUS_PROCESS).cast(),
                size_of::<SERVICE_STATUS_PROCESS>() as u32,
                &mut needed,
            )
        } != 0,
        "SCM status query failed"
    );
    Ok(ServiceInfo {
        image: unsafe { string(config.lpBinaryPathName)? },
        account: unsafe { string(config.lpServiceStartName)? },
        state: state.dwCurrentState,
        pid: state.dwProcessId,
        start_type: config.dwStartType,
    })
}
pub fn status() -> Result<serde_json::Value> {
    Ok(match Native.inspect()? {
        Some(info) => serde_json::json!({"state":"installed","service":info}),
        None => serde_json::json!({"state":"not_installed"}),
    })
}
pub(super) trait Controller {
    fn inspect(&self) -> Result<Option<ServiceInfo>>;
    fn create(&mut self, image: &str) -> Result<()>;
    fn configure(&mut self, image: &str) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn delete(&mut self) -> Result<()>;
}
pub(super) struct Native;
impl Controller for Native {
    fn inspect(&self) -> Result<Option<ServiceInfo>> {
        open(SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS)?
            .as_ref()
            .map(inspect_handle)
            .transpose()
    }
    fn create(&mut self, image: &str) -> Result<()> {
        let manager = manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)?;
        let h = unsafe {
            CreateServiceW(
                manager.0,
                wide(SERVICE_NAME).as_ptr(),
                wide("Dao-Shell File Index").as_ptr(),
                SERVICE_CHANGE_CONFIG | SERVICE_QUERY_STATUS | WRITE_DAC | DELETE,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                wide(image).as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                wide("LocalSystem").as_ptr(),
                ptr::null(),
            )
        };
        ensure!(
            !h.is_null(),
            "service creation failed: {}",
            std::io::Error::last_os_error()
        );
        let h = Handle(h);
        finish_creation(configure_security(&h), || {
            ensure!(
                unsafe { DeleteService(h.0) } != 0,
                "new-service cleanup failed"
            );
            Ok(())
        })
    }
    fn configure(&mut self, image: &str) -> Result<()> {
        let h = open(SERVICE_CHANGE_CONFIG | WRITE_DAC)?
            .ok_or_else(|| anyhow::anyhow!("service missing"))?;
        ensure!(
            unsafe {
                ChangeServiceConfigW(
                    h.0,
                    SERVICE_WIN32_OWN_PROCESS,
                    SERVICE_AUTO_START,
                    SERVICE_ERROR_NORMAL,
                    wide(image).as_ptr(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null(),
                    wide("LocalSystem").as_ptr(),
                    ptr::null(),
                    ptr::null(),
                )
            } != 0,
            "service configuration failed"
        );
        configure_security(&h)
    }
    fn start(&mut self) -> Result<()> {
        let h = open(SERVICE_START | SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)?
            .ok_or_else(|| anyhow::anyhow!("service missing"))?;
        if unsafe { StartServiceW(h.0, 0, ptr::null()) } == 0 {
            ensure!(
                unsafe { GetLastError() } == ERROR_SERVICE_ALREADY_RUNNING,
                "service start failed"
            );
        }
        wait(&h, SERVICE_RUNNING)
    }
    fn stop(&mut self) -> Result<()> {
        let Some(h) = open(SERVICE_STOP | SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)? else {
            return Ok(());
        };
        let mut state = SERVICE_STATUS::default();
        if unsafe { ControlService(h.0, SERVICE_CONTROL_STOP, &mut state) } == 0 {
            ensure!(
                unsafe { GetLastError() } == ERROR_SERVICE_NOT_ACTIVE,
                "service stop failed"
            );
        }
        wait(&h, SERVICE_STOPPED)
    }
    fn delete(&mut self) -> Result<()> {
        let Some(h) = open(DELETE)? else {
            return Ok(());
        };
        ensure!(
            unsafe { DeleteService(h.0) } != 0,
            "service deletion failed"
        );
        Ok(())
    }
}
fn finish_creation(setup: Result<()>, cleanup: impl FnOnce() -> Result<()>) -> Result<()> {
    if let Err(error) = setup {
        cleanup()?;
        return Err(error);
    }
    Ok(())
}
fn wait(h: &Handle, target: u32) -> Result<()> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let status = inspect_handle(h)?;
        if status.state == target {
            return Ok(());
        }
        ensure!(Instant::now() < until, "SCM transition timed out");
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn configure_security(h: &Handle) -> Result<()> {
    use windows_sys::Win32::Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
        DACL_SECURITY_INFORMATION,
    };
    // Ordinary users can verify SCM identity, but cannot change, stop or start the service.
    let mut sd = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;CCLCRC;;;AU)").as_ptr(),
                1,
                &mut sd,
                ptr::null_mut(),
            )
        } != 0,
        "service ACL invalid"
    );
    let sd = super::super::security::Descriptor(sd);
    ensure!(
        unsafe { SetServiceObjectSecurity(h.0, DACL_SECURITY_INFORMATION, sd.0) } != 0,
        "service ACL update failed"
    );
    let mut names = wide("SeImpersonatePrivilege");
    names.push(0);
    let privileges = SERVICE_REQUIRED_PRIVILEGES_INFOW {
        pmszRequiredPrivileges: names.as_mut_ptr(),
    };
    ensure!(
        unsafe {
            ChangeServiceConfig2W(
                h.0,
                SERVICE_CONFIG_REQUIRED_PRIVILEGES_INFO,
                (&privileges as *const SERVICE_REQUIRED_PRIVILEGES_INFOW).cast(),
            )
        } != 0,
        "service privilege restriction failed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_security_configuration_cleans_created_service_and_reports_cleanup_failure() {
        let removed = std::cell::Cell::new(false);
        assert!(
            finish_creation(
                Err(anyhow::anyhow!("privilege configuration failed")),
                || {
                    removed.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(removed.get());
        let error = finish_creation(Err(anyhow::anyhow!("ACL failure")), || {
            Err(anyhow::anyhow!("new-service cleanup failed"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "new-service cleanup failed");
        assert!(finish_creation(Ok(()), || panic!("healthy service must not be deleted")).is_ok());
    }
}
