//! A folder picker only: no file reads, recursive scanning, or permission changes.
#[cfg(windows)]
pub fn choose(owner: isize) -> Result<Option<String>, String> {
    use windows::Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::Shell::{
            FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
            IFileOpenDialog, SIGDN_FILESYSPATH,
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // Balanced with the successful initialization on this same blocking thread.
            unsafe { CoUninitialize() };
        }
    }
    // The COM objects stay on this thread and are released before its apartment.
    let result = unsafe {
        (|| -> windows::core::Result<Option<String>> {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let _apartment = Apartment;
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
            if let Err(error) = dialog.Show(Some(HWND(owner as *mut _))) {
                if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(error);
            }
            let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = name.to_string();
            CoTaskMemFree(Some(name.0.cast()));
            Ok(Some(path?))
        })()
    };
    result.map_err(|e| format!("系统目录选择失败：{e}"))
}

#[cfg(not(windows))]
pub fn choose(_: isize) -> Result<Option<String>, String> {
    Err("此预览版的目录选择仅支持 Windows".into())
}
