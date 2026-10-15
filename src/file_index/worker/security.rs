use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsStr,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Pipes::*, SystemServices::SE_GROUP_ENABLED, Threading::*},
};
pub(super) fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
pub(super) fn owned(raw: HANDLE) -> Result<OwnedHandle> {
    ensure!(
        raw != INVALID_HANDLE_VALUE && !raw.is_null(),
        "Win32 handle: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { OwnedHandle::from_raw_handle(raw as _) })
}
pub(super) fn raw(h: &OwnedHandle) -> HANDLE {
    h.as_raw_handle() as _
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Principal {
    pub sid: String,
    pub session: u32,
    pub pid: u32,
    pub elevated: bool,
    pub authentication_id: u64,
    pub interactive: bool,
}
fn token_buffer(token: HANDLE, kind: TOKEN_INFORMATION_CLASS) -> Result<Vec<u8>> {
    let mut size = 0;
    unsafe { GetTokenInformation(token, kind, ptr::null_mut(), 0, &mut size) };
    ensure!((4..=65536).contains(&size), "invalid token size");
    let mut buffer = vec![0u8; size as usize];
    ensure!(
        unsafe { GetTokenInformation(token, kind, buffer.as_mut_ptr().cast(), size, &mut size) }
            != 0,
        "token query failed"
    );
    Ok(buffer)
}
fn principal(token: HANDLE, pid: u32) -> Result<Principal> {
    let user = token_buffer(token, TokenUser)?;
    let info = unsafe { ptr::read_unaligned(user.as_ptr().cast::<TOKEN_USER>()) };
    let mut string = ptr::null_mut();
    ensure!(
        unsafe { ConvertSidToStringSidW(info.User.Sid, &mut string) } != 0,
        "SID conversion failed"
    );
    let sid = unsafe {
        let mut len = 0;
        while *string.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16(std::slice::from_raw_parts(string, len))?;
        LocalFree(string.cast());
        s
    };
    let session = token_buffer(token, TokenSessionId)?;
    let elevation = token_buffer(token, TokenElevation)?;
    let statistics = token_buffer(token, TokenStatistics)?;
    let statistics = unsafe { ptr::read_unaligned(statistics.as_ptr().cast::<TOKEN_STATISTICS>()) };
    let groups = token_buffer(token, TokenGroups)?;
    let group_count = u32::from_ne_bytes(groups[..4].try_into()?) as usize;
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    ensure!(
        offset + group_count * size_of::<SID_AND_ATTRIBUTES>() <= groups.len(),
        "invalid token groups"
    );
    let interactive = (0..group_count).any(|index| {
        let group = unsafe {
            ptr::read_unaligned(
                groups
                    .as_ptr()
                    .add(offset + index * size_of::<SID_AND_ATTRIBUTES>())
                    .cast::<SID_AND_ATTRIBUTES>(),
            )
        };
        group.Attributes & SE_GROUP_ENABLED as u32 != 0
            && unsafe { IsWellKnownSid(group.Sid, WinInteractiveSid) } != 0
    });
    Ok(Principal {
        sid,
        session: u32::from_ne_bytes(session[..4].try_into()?),
        pid,
        elevated: u32::from_ne_bytes(elevation[..4].try_into()?) != 0,
        authentication_id: ((statistics.AuthenticationId.HighPart as u64) << 32)
            | statistics.AuthenticationId.LowPart as u64,
        interactive,
    })
}
pub(super) fn current() -> Result<Principal> {
    let mut token = ptr::null_mut();
    ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0,
        "process token failed"
    );
    let token = owned(token)?;
    principal(raw(&token), std::process::id())
}
pub(super) struct Impersonation;
impl Drop for Impersonation {
    fn drop(&mut self) {
        if unsafe { RevertToSelf() } == 0 {
            std::process::abort();
        }
    }
}
pub(super) fn impersonate(pipe: HANDLE) -> Result<Impersonation> {
    ensure!(
        unsafe { ImpersonateNamedPipeClient(pipe) } != 0,
        "pipe impersonation failed"
    );
    Ok(Impersonation)
}
pub(super) fn caller(pipe: HANDLE, owner: &Principal) -> Result<Principal> {
    let actor = pipe_actor(pipe)?;
    ensure!(
        actor.sid == owner.sid && actor.session == owner.session,
        "client identity denied"
    );
    Ok(actor)
}
pub(super) fn service_caller(pipe: HANDLE, owner_sid: &str) -> Result<Principal> {
    let actor = pipe_actor(pipe)?;
    ensure!(
        actor.sid == owner_sid && actor.interactive && actor.session > 0,
        "installation owner interactive logon required"
    );
    Ok(actor)
}
fn pipe_actor(pipe: HANDLE) -> Result<Principal> {
    let mut pid = 0;
    let mut session = 0;
    ensure!(
        unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } != 0
            && unsafe { GetNamedPipeClientSessionId(pipe, &mut session) } != 0,
        "pipe client identity failed"
    );
    let _guard = impersonate(pipe)?;
    let mut token = ptr::null_mut();
    ensure!(
        unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) } != 0,
        "impersonation token failed"
    );
    let token = owned(token)?;
    let actor = principal(raw(&token), pid)?;
    ensure!(actor.session == session, "client identity denied");
    Ok(actor)
}
pub(super) fn installation_principal() -> Result<Principal> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};
    let actor = current()?;
    ensure!(
        actor.elevated && actor.interactive && actor.session > 0,
        "interactive elevated installer required"
    );
    let shell = unsafe { GetShellWindow() };
    ensure!(
        !shell.is_null(),
        "interactive shell unavailable: unsupported installation identity"
    );
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(shell, &mut pid) };
    let process = owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
    let mut token = ptr::null_mut();
    ensure!(
        unsafe { OpenProcessToken(raw(&process), TOKEN_QUERY, &mut token) } != 0,
        "interactive shell token unavailable"
    );
    let token = owned(token)?;
    let user = principal(raw(&token), pid)?;
    ensure!(
        user.sid == actor.sid && user.session == actor.session && user.interactive,
        "cross-account installation unsupported"
    );
    Ok(actor)
}
pub(super) fn verify_server(
    pipe: HANDLE,
    expected_image: &Path,
    allow_fixture: bool,
) -> Result<()> {
    let own = current()?;
    let mut pid = 0;
    ensure!(
        unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) } != 0,
        "server PID unavailable"
    );
    let process = owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
    let mut token = ptr::null_mut();
    ensure!(
        unsafe { OpenProcessToken(raw(&process), TOKEN_QUERY, &mut token) } != 0,
        "server token unavailable"
    );
    let token = owned(token)?;
    let server = principal(raw(&token), pid)?;
    ensure!(
        server.sid == own.sid
            && server.session == own.session
            && (allow_fixture || server.elevated),
        "server identity mismatch"
    );
    let mut path = vec![0u16; 32768];
    let mut len = path.len() as u32;
    ensure!(
        unsafe { QueryFullProcessImageNameW(raw(&process), 0, path.as_mut_ptr(), &mut len) } != 0,
        "server image unavailable"
    );
    let actual = PathBuf::from(String::from_utf16(&path[..len as usize])?);
    ensure!(
        actual.canonicalize()? == expected_image.canonicalize()?,
        "server image mismatch"
    );
    Ok(())
}
pub(super) struct Descriptor(pub PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
pub(super) fn descriptor(owner: &Principal, fixture: bool) -> Result<Descriptor> {
    // Exclude FILE_CREATE_PIPE_INSTANCE from the ordinary client's rights.
    let mask = if fixture { "FA" } else { "0x00120183" };
    let sddl = format!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;{mask};;;{})", owner.sid);
    let mut value = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(&sddl).as_ptr(),
                1,
                &mut value,
                ptr::null_mut(),
            )
        } != 0,
        "pipe ACL creation failed"
    );
    Ok(Descriptor(value))
}
pub(super) fn visible(root: &Path, components: &[String], expected_id: &str) -> bool {
    fn check(root: &Path, components: &[String], id: &str) -> Result<()> {
        ensure!(
            !components.is_empty() && components.len() <= 256,
            "invalid candidate"
        );
        let mut held = Vec::new();
        let mut path = root.to_path_buf();
        let mut serial = None;
        for index in 0..=components.len() {
            if index > 0 {
                let component = &components[index - 1];
                ensure!(
                    !component.is_empty()
                        && !component.contains(['\\', '/', '\0', ':'])
                        && component != "."
                        && component != "..",
                    "invalid component"
                );
                path.push(component);
            }
            let parent = index < components.len();
            let access = FILE_READ_ATTRIBUTES | if parent { FILE_LIST_DIRECTORY } else { 0 };
            // Prevent ancestor rename/replacement during this authorization check.
            let file = owned(unsafe {
                CreateFileW(
                    wide(path.to_str().context("non Unicode path")?).as_ptr(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    ptr::null_mut(),
                )
            })?;
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            ensure!(
                unsafe { GetFileInformationByHandle(raw(&file), &mut info) } != 0,
                "candidate metadata denied"
            );
            if let Some(s) = serial {
                ensure!(s == info.dwVolumeSerialNumber, "candidate volume changed");
            } else {
                serial = Some(info.dwVolumeSerialNumber);
            }
            if parent {
                ensure!(
                    info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
                        && info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                    "unsafe ancestor"
                );
            } else {
                let native = ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64;
                ensure!(format!("{native:016x}") == id, "candidate replaced");
            }
            held.push(file);
        }
        Ok(())
    }
    check(root, components, expected_id).is_ok()
}

#[cfg(test)]
mod service_identity_tests {
    use super::*;
    use crate::file_index::{Cancellation, worker::transport};
    #[test]
    fn real_pipe_caller_is_bound_to_token_sid_and_logon_not_request_body() {
        let actor = current().unwrap();
        assert!(actor.interactive);
        assert!(actor.session > 0);
        let name = format!("service-caller-{}", uuid::Uuid::new_v4().simple());
        let mut pipes = transport::listeners(&name, true).unwrap();
        let pipe = pipes.remove(0);
        let expected = actor.clone();
        let server = std::thread::spawn(move || {
            let timeout = Cancellation::with_timeout(std::time::Duration::from_secs(3));
            transport::accept(raw(&pipe), &timeout).unwrap();
            let _untrusted = transport::receive(raw(&pipe), &timeout).unwrap();
            assert!(service_caller(raw(&pipe), "S-1-5-21-0-0-0-1234").is_err());
            let actual = service_caller(raw(&pipe), &expected.sid).unwrap();
            assert_eq!(actual.sid, expected.sid);
            assert_eq!(actual.authentication_id, expected.authentication_id);
            assert_eq!(actual.session, expected.session);
            transport::send(raw(&pipe), b"verified", &timeout).unwrap();
            transport::wait_ack(raw(&pipe), &timeout).unwrap();
        });
        let timeout = Cancellation::with_timeout(std::time::Duration::from_secs(3));
        let client =
            transport::connect(&name, &std::env::current_exe().unwrap(), true, &timeout).unwrap();
        transport::send(raw(&client), br#"{"owner_sid":"S-1-5-18"}"#, &timeout).unwrap();
        assert_eq!(
            transport::receive(raw(&client), &timeout).unwrap(),
            b"verified"
        );
        transport::acknowledge(raw(&client), &timeout).unwrap();
        server.join().unwrap();
    }
}
