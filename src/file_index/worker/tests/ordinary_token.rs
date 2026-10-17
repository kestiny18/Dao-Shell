//! Run ordinary-token IPC fixtures even when the test runner itself is elevated.
use super::security;
use anyhow::{Context, Result, ensure};
use std::{fs::File, os::windows::io::AsRawHandle, ptr};
use windows_sys::Win32::{Foundation::*, Security::*, System::Threading::*};

pub(super) fn ordinary_child(name: &str) -> bool {
    if !security::current().unwrap().elevated {
        return false;
    }
    run(name).unwrap();
    true
}

pub(super) fn run(name: &str) -> Result<()> {
    ensure!(
        name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "invalid fixture name"
    );
    let mut token = ptr::null_mut();
    ensure!(
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
                &mut token,
            )
        } != 0,
        "open fixture token: {}",
        std::io::Error::last_os_error()
    );
    let token = security::owned(token)?;
    let mut limited = ptr::null_mut();
    ensure!(
        unsafe {
            CreateRestrictedToken(
                security::raw(&token),
                LUA_TOKEN | DISABLE_MAX_PRIVILEGE,
                0,
                ptr::null(),
                0,
                ptr::null(),
                0,
                ptr::null(),
                &mut limited,
            )
        } != 0,
        "restrict fixture token: {}",
        std::io::Error::last_os_error()
    );
    let limited = security::owned(limited)?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut size = 0;
    ensure!(
        unsafe {
            GetTokenInformation(
                security::raw(&limited),
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
        } != 0,
        "query restricted fixture elevation"
    );
    ensure!(
        elevation.TokenIsElevated == 0,
        "fixture child token is still elevated"
    );
    let dir = tempfile::tempdir()?;
    let log = dir.path().join("child.log");
    let output = File::create(&log)?;
    let handle = output.as_raw_handle() as HANDLE;
    let input = File::open("NUL")?;
    let input_handle = input.as_raw_handle() as HANDLE;
    ensure!(
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } != 0,
        "inherit fixture output"
    );
    ensure!(
        unsafe { SetHandleInformation(input_handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }
            != 0,
        "inherit fixture input"
    );
    // A NULL desktop inherits the elevated parent's window station/desktop. An
    // empty name lets Windows connect using the restricted child's logon context.
    let mut desktop = [0u16];
    // Only this child's valid standard handles may be inherited. In particular,
    // do not inherit another parallel fixture's desktop/window-station handles.
    let mut bytes = 0;
    unsafe {
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes);
    }
    ensure!(bytes > 0, "fixture attribute size unavailable");
    let mut attribute_storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let attributes = attribute_storage.as_mut_ptr().cast();
    ensure!(
        unsafe { InitializeProcThreadAttributeList(attributes, 1, 0, &mut bytes) } != 0,
        "initialize fixture handle list"
    );
    struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for Attributes {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.0);
            }
        }
    }
    let _attributes = Attributes(attributes);
    let inherited = [handle, input_handle];
    ensure!(
        unsafe {
            UpdateProcThreadAttribute(
                attributes,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                inherited.as_ptr().cast(),
                size_of_val(&inherited),
                ptr::null_mut(),
                ptr::null(),
            )
        } != 0,
        "set fixture handle list"
    );
    let exe = std::env::current_exe()?;
    let exe = security::wide(exe.to_str().context("non Unicode test image")?);
    let mut command = security::wide(&format!(
        "\"{}\" --exact file_index::worker::tests::{name} --nocapture",
        std::env::current_exe()?.display()
    ));
    let startup = STARTUPINFOEXW {
        StartupInfo: STARTUPINFOW {
            cb: size_of::<STARTUPINFOEXW>() as u32,
            lpDesktop: desktop.as_mut_ptr(),
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: input_handle,
            hStdOutput: handle,
            hStdError: handle,
            ..Default::default()
        },
        lpAttributeList: attributes,
    };
    let mut info = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessAsUserW(
            security::raw(&limited),
            exe.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            ptr::null(),
            &startup.StartupInfo,
            &mut info,
        )
    };
    let error = std::io::Error::last_os_error();
    unsafe {
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
        SetHandleInformation(input_handle, HANDLE_FLAG_INHERIT, 0);
    }
    ensure!(created != 0, "start restricted fixture child: {error}");
    let process = security::owned(info.hProcess)?;
    let _thread = security::owned(info.hThread)?;
    struct Child(HANDLE);
    impl Drop for Child {
        fn drop(&mut self) {
            // Only the exact child handle created above can be stopped on timeout/error.
            unsafe {
                if WaitForSingleObject(self.0, 0) != WAIT_OBJECT_0 {
                    TerminateProcess(self.0, 1);
                    WaitForSingleObject(self.0, 5000);
                }
            }
        }
    }
    let _child = Child(security::raw(&process));
    let wait = unsafe { WaitForSingleObject(security::raw(&process), 30000) };
    ensure!(
        wait == WAIT_OBJECT_0,
        "restricted fixture child did not finish: {wait}"
    );
    let mut code = 0;
    ensure!(
        unsafe { GetExitCodeProcess(security::raw(&process), &mut code) } != 0,
        "child exit code unavailable"
    );
    let text = std::fs::read_to_string(log)?;
    ensure!(
        code == 0,
        "restricted fixture {name} failed ({code:#x}):\n{text}"
    );
    ensure!(
        text.contains("1 passed"),
        "restricted fixture did not run its exact test: {text}"
    );
    Ok(())
}
