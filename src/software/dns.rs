//! Fixed DNS resolver-cache operation. No server, adapter or proxy settings are changed.
use anyhow::Result;
#[cfg(windows)]
use anyhow::ensure;
#[cfg(windows)]
pub fn flush() -> Result<serde_json::Value> {
    use windows_sys::Win32::System::{LibraryLoader::*, SystemInformation::GetSystemDirectoryW};
    let mut system = [0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    ensure!(len > 0 && len < system.len(), "dns_provider_unavailable");
    let mut path = system[..len].to_vec();
    path.extend("\\dnsapi.dll".encode_utf16());
    path.push(0);
    let library = unsafe {
        LoadLibraryExW(
            path.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    ensure!(!library.is_null(), "dns_provider_unavailable");
    struct Library(windows_sys::Win32::Foundation::HMODULE);
    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::Foundation::FreeLibrary(self.0);
            }
        }
    }
    let _library = Library(library);
    let symbol = unsafe { GetProcAddress(library, c"DnsFlushResolverCache".as_ptr().cast()) }
        .ok_or_else(|| anyhow::anyhow!("dns_provider_unavailable"))?;
    let call: unsafe extern "system" fn() -> i32 = unsafe { std::mem::transmute(symbol) };
    let ok = unsafe { call() } != 0;
    Ok(
        serde_json::json!({"lifecycle":if ok{"verified"}else{"failed"},"provider":"dnsapi_fixed_flush","cache_clear_reported":ok,"connectivity_verified":false,"dns_server_changed":false,"restart_requested":false}),
    )
}
#[cfg(not(windows))]
pub fn flush() -> Result<serde_json::Value> {
    anyhow::bail!("unsupported_platform")
}
