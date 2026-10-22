//! Owned, hidden-window T0 probe. Never enumerates or messages another program.
//! cargo run --example window_close_boundary -- <NEW report.json>
#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use serde_json::json;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
    };
    static REQUESTS: AtomicUsize = AtomicUsize::new(0);
    static IGNORE: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_CLOSE {
            REQUESTS.fetch_add(1, Ordering::SeqCst);
            if IGNORE.load(Ordering::SeqCst) == 0 {
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    let report = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("A NEW report path is required"))?,
    );
    anyhow::ensure!(!report.exists(), "Refusing to overwrite a report");
    let class = wide(&format!("DaoShellOwnedT0-{}", std::process::id()));
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let definition = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    anyhow::ensure!(
        unsafe { RegisterClassW(&definition) } != 0,
        "register owned class"
    );
    let create = || unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            100,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    let pump = || {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    };
    let first = create();
    anyhow::ensure!(!first.is_null(), "create own window");
    anyhow::ensure!(
        unsafe { PostMessageW(first, WM_CLOSE, 0, 0) } != 0,
        "post to owned window"
    );
    pump();
    anyhow::ensure!(
        unsafe { IsWindow(first) } == 0 && REQUESTS.load(Ordering::SeqCst) == 1,
        "close evidence"
    );
    IGNORE.store(1, Ordering::SeqCst);
    let ignored = create();
    anyhow::ensure!(
        unsafe { PostMessageW(ignored, WM_CLOSE, 0, 0) } != 0,
        "post ignored close"
    );
    pump();
    anyhow::ensure!(
        unsafe { IsWindow(ignored) } != 0 && REQUESTS.load(Ordering::SeqCst) == 2,
        "requested does not imply closed"
    );
    let mut expected_pid = 0;
    unsafe {
        GetWindowThreadProcessId(ignored, &mut expected_pid);
    }
    anyhow::ensure!(expected_pid == std::process::id(), "owned only");
    // Scheduled destruction after a successful ownership check. Never message the
    // stale numeric handle: another unrelated window could already own it.
    unsafe {
        DestroyWindow(ignored);
    }
    let stale_rejected = unsafe { IsWindow(ignored) } == 0;
    anyhow::ensure!(
        stale_rejected,
        "destroyed HWND was not recognized as unavailable"
    );
    // Native bounded generation-reuse probe; absence is not a safety proof.
    let began = Instant::now();
    let mut reused = false;
    let mut iterations = 0usize;
    while began.elapsed() < Duration::from_secs(5) && iterations < 200_000 {
        let next = create();
        anyhow::ensure!(!next.is_null(), "fixture create");
        iterations += 1;
        if next == ignored {
            reused = true;
            unsafe {
                DestroyWindow(next);
            }
            break;
        }
        unsafe {
            DestroyWindow(next);
        }
    }
    unsafe {
        UnregisterClassW(class.as_ptr(), instance);
    }
    let value = json!({"owned_windows_only":true,"post_success_did_not_mean_closed":true,"destroy_after_check_rejected":stale_rejected,"native_reuse_observed":reused,"native_iterations":iterations,"elapsed_ms":began.elapsed().as_millis(),"atomic_identity_and_dispatch_proven":false,"normal_close_supported":false,"reason":"Win32 validates a numeric HWND at dispatch, without a caller-supplied expected process/window generation. Preflight identity checks cannot make the final send conditional atomically. Native stress is a bounded observation, not proof of absence."});
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report)?;
    use std::io::Write;
    file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
    println!("Owned-window boundary probe complete; atomic dispatch remains unproven.");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows-only owned-window probe");
}
