//! The OS loader boot sequence, not wall-clock uptime or a BCD entry identifier.
//! Windows documents BootId in KUSER_SHARED_DATA and its user read-only mapping:
//! https://learn.microsoft.com/windows-hardware/drivers/ddi/ntddk/ns-ntddk-kuser_shared_data
//! https://www.microsoft.com/msrc/blog/2022/04/randomizing-the-kuser_shared_data-structure-on-windows/
#[cfg(all(windows, target_arch = "x86_64"))]
pub fn sequence() -> Option<u32> {
    // Win10/11 x64 ABI: BootId at offset 0x2c4 of the fixed user mapping.
    // ReadProcessMemory fails safely if a future OS no longer exposes this layout.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ReadProcessMemory(
            process: *mut std::ffi::c_void,
            base: *const std::ffi::c_void,
            buffer: *mut std::ffi::c_void,
            size: usize,
            read: *mut usize,
        ) -> i32;
    }
    let mut value = 0u32;
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            -1isize as _,
            0x7ffe02c4usize as _,
            (&mut value as *mut u32).cast(),
            4,
            &mut read,
        )
    };
    (ok != 0 && read == 4 && value != 0).then_some(value)
}
#[cfg(not(all(windows, target_arch = "x86_64")))]
pub fn sequence() -> Option<u32> {
    None
}
