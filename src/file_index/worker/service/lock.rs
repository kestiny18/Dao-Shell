use super::super::security::{Descriptor, owned, raw, wide};
use anyhow::{Result, ensure};
use std::{os::windows::io::OwnedHandle, ptr};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::Threading::*,
};

pub(super) struct InstallerLock(OwnedHandle);
impl Drop for InstallerLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(raw(&self.0));
        }
    }
}
fn acquire(name: &str) -> Result<InstallerLock> {
    let mut descriptor = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide("O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)").as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } != 0,
        "installer lock ACL failed"
    );
    let _descriptor = Descriptor(descriptor);
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    unsafe {
        SetLastError(0);
    }
    let handle = unsafe { CreateMutexW(&attrs, 1, wide(name).as_ptr()) };
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let handle = owned(handle)?;
    // Never adopt a pre-created mutex, even if its ACL permits access.
    ensure!(
        !existed,
        "another installer or reserved-name collision; no changes made"
    );
    Ok(InstallerLock(handle))
}
pub(super) fn installation() -> Result<InstallerLock> {
    acquire(r"Global\DaoShellIndexInstaller-51b0edc0-220d-4e5b-86a8-8cb6e57e6157")
}
