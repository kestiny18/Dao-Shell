use super::{
    protocol::MAX_MESSAGE,
    security::{self, owned, raw, wide},
};
use crate::file_index::Cancellation;
use anyhow::{Result, ensure};
use std::{os::windows::io::OwnedHandle, path::Path, ptr, time::Duration};
use windows_sys::Win32::{
    Foundation::*, Security::SECURITY_ATTRIBUTES, Storage::FileSystem::*, System::Pipes::*,
};
pub(super) fn pipe_name(name: &str) -> Result<Vec<u16>> {
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c)),
        "invalid pipe name"
    );
    Ok(wide(&format!(r"\\.\pipe\DaoShellIndex-{name}")))
}
pub(super) fn listeners(name: &str, fixture: bool) -> Result<Vec<OwnedHandle>> {
    let owner = security::current()?;
    ensure!(fixture || owner.elevated, "worker requires elevated token");
    listeners_for(name, &owner, fixture)
}
pub(super) fn listeners_for(
    name: &str,
    owner: &security::Principal,
    fixture: bool,
) -> Result<Vec<OwnedHandle>> {
    let descriptor = security::descriptor(owner, fixture)?;
    let attr = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let name = pipe_name(name)?;
    let mut pipes = Vec::new();
    for i in 0..4 {
        pipes.push(owned(unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX
                    | if i == 0 {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        0
                    },
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                4,
                MAX_MESSAGE as u32,
                MAX_MESSAGE as u32,
                1000,
                &attr,
            )
        })?);
    }
    Ok(pipes)
}
pub(super) fn accept(pipe: HANDLE, stop: &Cancellation) -> Result<()> {
    loop {
        stop.check()?;
        let ok = unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) };
        if ok != 0 {
            return Ok(());
        }
        let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
        match code {
            ERROR_PIPE_CONNECTED => return Ok(()),
            ERROR_PIPE_LISTENING => (),
            ERROR_NO_DATA => unsafe {
                DisconnectNamedPipe(pipe);
            },
            _ => anyhow::bail!("connect pipe error {code}"),
        };
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn read_exact(pipe: HANDLE, out: &mut [u8], cancel: &Cancellation) -> Result<()> {
    let mut done = 0;
    while done < out.len() {
        cancel.check()?;
        let mut n = 0;
        let ok = unsafe {
            ReadFile(
                pipe,
                out[done..].as_mut_ptr(),
                (out.len() - done) as u32,
                &mut n,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
            ensure!(code == ERROR_NO_DATA, "pipe read failed");
            let mut available = 0;
            ensure!(
                unsafe {
                    PeekNamedPipe(
                        pipe,
                        ptr::null_mut(),
                        0,
                        ptr::null_mut(),
                        &mut available,
                        ptr::null_mut(),
                    )
                } != 0,
                "pipe disconnected"
            );
        }
        done += n as usize;
        if n == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    Ok(())
}
fn write_all(pipe: HANDLE, data: &[u8], cancel: &Cancellation) -> Result<()> {
    let mut done = 0;
    while done < data.len() {
        cancel.check()?;
        let mut n = 0;
        ensure!(
            unsafe {
                WriteFile(
                    pipe,
                    data[done..].as_ptr(),
                    (data.len() - done) as u32,
                    &mut n,
                    ptr::null_mut(),
                )
            } != 0,
            "pipe write failed"
        );
        done += n as usize;
        if n == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    Ok(())
}
pub(super) fn receive(pipe: HANDLE, cancel: &Cancellation) -> Result<Vec<u8>> {
    let mut size = [0u8; 4];
    read_exact(pipe, &mut size, cancel)?;
    let size = u32::from_le_bytes(size) as usize;
    ensure!(size > 0 && size <= MAX_MESSAGE, "oversized message");
    let mut bytes = vec![0; size];
    read_exact(pipe, &mut bytes, cancel)?;
    Ok(bytes)
}
pub(super) fn send(pipe: HANDLE, bytes: &[u8], cancel: &Cancellation) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_MESSAGE,
        "oversized message"
    );
    write_all(pipe, &(bytes.len() as u32).to_le_bytes(), cancel)?;
    write_all(pipe, bytes, cancel)
}
pub(super) fn acknowledge(pipe: HANDLE, cancel: &Cancellation) -> Result<()> {
    write_all(pipe, &[1], cancel)
}
pub(super) fn wait_ack(pipe: HANDLE, cancel: &Cancellation) -> Result<()> {
    let mut ack = [0];
    read_exact(pipe, &mut ack, cancel)?;
    ensure!(ack == [1], "bad acknowledgement");
    Ok(())
}
pub(super) fn disconnect(pipe: HANDLE) {
    unsafe {
        DisconnectNamedPipe(pipe);
    }
}
pub(super) fn connect(
    name: &str,
    image: &Path,
    fixture: bool,
    cancel: &Cancellation,
) -> Result<OwnedHandle> {
    let pipe = connect_unverified(name, cancel)?;
    security::verify_server(raw(&pipe), image, fixture)?;
    Ok(pipe)
}
pub(super) fn connect_unverified(name: &str, cancel: &Cancellation) -> Result<OwnedHandle> {
    let name = pipe_name(name)?;
    loop {
        cancel.check()?;
        let pipe = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_READ_DATA
                    | FILE_WRITE_DATA
                    | FILE_READ_ATTRIBUTES
                    | FILE_WRITE_ATTRIBUTES
                    | READ_CONTROL
                    | SYNCHRONIZE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION,
                ptr::null_mut(),
            )
        };
        if pipe != INVALID_HANDLE_VALUE {
            let pipe = owned(pipe)?;
            let mode = PIPE_NOWAIT | PIPE_READMODE_BYTE;
            ensure!(
                unsafe { SetNamedPipeHandleState(raw(&pipe), &mode, ptr::null(), ptr::null()) }
                    != 0,
                "client pipe mode failed"
            );
            return Ok(pipe);
        }
        let code = std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
        ensure!(
            code == ERROR_PIPE_BUSY || code == ERROR_FILE_NOT_FOUND,
            "pipe connection denied"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
