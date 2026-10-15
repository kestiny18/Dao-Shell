//! Authenticate the connected pipe object without opening the SYSTEM process.
use super::super::security;
use anyhow::{Result, ensure};
use std::ptr;
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Pipes::*,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Peer {
    pub pid: u32,
    pub session: u32,
}
pub(super) fn identity(pipe: HANDLE) -> Result<Peer> {
    let mut peer = Peer {
        pid: 0,
        session: u32::MAX,
    };
    ensure!(
        unsafe { GetNamedPipeServerProcessId(pipe, &mut peer.pid) } != 0
            && unsafe { GetNamedPipeServerSessionId(pipe, &mut peer.session) } != 0,
        "pipe peer identity unavailable"
    );
    Ok(peer)
}
fn sid(sid: PSID) -> Result<String> {
    ensure!(
        !sid.is_null() && unsafe { IsValidSid(sid) } != 0,
        "invalid pipe SID"
    );
    let mut text = ptr::null_mut();
    ensure!(
        unsafe { ConvertSidToStringSidW(sid, &mut text) } != 0,
        "pipe SID unavailable"
    );
    let result = unsafe {
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, n))
    };
    unsafe {
        LocalFree(text.cast());
    }
    Ok(result?)
}
pub(super) fn verify(pipe: HANDLE, reader: &str) -> Result<()> {
    let mut descriptor = ptr::null_mut();
    let code = unsafe {
        GetSecurityInfo(
            pipe,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    ensure!(code == 0, "pipe security descriptor unavailable: {code}");
    let held = security::Descriptor(descriptor);
    verify_descriptor(held.0, reader)
}
fn verify_descriptor(descriptor: PSECURITY_DESCRIPTOR, reader: &str) -> Result<()> {
    let mut owner = ptr::null_mut();
    let mut defaulted = 0;
    ensure!(
        unsafe { GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted) } != 0,
        "pipe owner unavailable"
    );
    ensure!(
        matches!(sid(owner)?.as_str(), "S-1-5-18" | "S-1-5-32-544"),
        "untrusted pipe owner"
    );
    let mut control = 0;
    let mut revision = 0;
    ensure!(
        unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } != 0
            && control & SE_DACL_PROTECTED != 0,
        "unprotected pipe DACL"
    );
    let mut present = 0;
    let mut acl = ptr::null_mut();
    ensure!(
        unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) }
            != 0
            && present != 0
            && !acl.is_null(),
        "missing pipe DACL"
    );
    let mut size = ACL_SIZE_INFORMATION::default();
    ensure!(
        unsafe {
            GetAclInformation(
                acl,
                (&mut size as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } != 0
            && size.AceCount == 3,
        "unexpected pipe ACL"
    );
    let mut actual = Vec::new();
    for index in 0..size.AceCount {
        let mut value = ptr::null_mut();
        ensure!(
            unsafe { GetAce(acl, index, &mut value) } != 0,
            "invalid pipe ACE"
        );
        let ace = unsafe { &*value.cast::<ACCESS_ALLOWED_ACE>() };
        ensure!(
            ace.Header.AceType == 0 && ace.Header.AceFlags == 0,
            "unexpected pipe ACE type"
        );
        let mut mask = ace.Mask;
        let mapping = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ,
            GenericWrite: FILE_GENERIC_WRITE,
            GenericExecute: FILE_GENERIC_EXECUTE,
            GenericAll: FILE_ALL_ACCESS,
        };
        unsafe {
            MapGenericMask(&mut mask, &mapping);
        }
        actual.push((sid((&ace.SidStart as *const u32).cast_mut().cast())?, mask));
    }
    actual.sort();
    let mut expected = vec![
        ("S-1-5-18".to_owned(), FILE_ALL_ACCESS),
        ("S-1-5-32-544".to_owned(), FILE_ALL_ACCESS),
        (reader.to_owned(), 0x00120183),
    ];
    expected.sort();
    ensure!(actual == expected, "unexpected pipe permissions");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn descriptor(sddl: &str) -> security::Descriptor {
        let mut value = ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    security::wide(sddl).as_ptr(),
                    1,
                    &mut value,
                    ptr::null_mut(),
                )
            },
            0
        );
        security::Descriptor(value)
    }
    #[test]
    fn descriptor_rejects_owner_null_unprotected_or_extra_rights() {
        let reader = "S-1-5-21-1-2-3-1001";
        let good = format!("O:BAD:P(A;;GA;;;SY)(A;;FA;;;BA)(A;;0x00120183;;;{reader})");
        assert!(verify_descriptor(descriptor(&good).0, reader).is_ok());
        for bad in [
            good.replace("O:BA", &format!("O:{reader}")),
            good.replace("D:P", "D:"),
            good.replace("0x00120183", "GWGR"),
            good.replace("0x00120183", "0x00120187"),
            good.replace("O:BA", "O:WD"),
            "O:BAD:NO_ACCESS_CONTROL".into(),
        ] {
            assert!(verify_descriptor(descriptor(&bad).0, reader).is_err());
        }
    }
    #[test]
    fn ordinary_pipe_owner_and_interactive_session_are_not_system_evidence() {
        use crate::file_index::worker::transport;
        let name = uuid::Uuid::new_v4().to_string();
        let reader = security::current().unwrap().sid;
        let mut pipes = transport::listeners(&name, true).unwrap();
        let pipe = pipes.pop().unwrap();
        drop(pipes);
        let server = std::thread::spawn(move || {
            transport::accept(
                security::raw(&pipe),
                &crate::file_index::Cancellation::with_timeout(std::time::Duration::from_secs(3)),
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(150));
        });
        let client = transport::connect_unverified(
            &name,
            &crate::file_index::Cancellation::with_timeout(std::time::Duration::from_secs(2)),
        )
        .unwrap();
        let peer = identity(security::raw(&client)).unwrap();
        assert_eq!(peer.pid, std::process::id());
        assert_ne!(peer.session, 0);
        assert!(verify(security::raw(&client), &reader).is_err());
        server.join().unwrap();
    }
}
