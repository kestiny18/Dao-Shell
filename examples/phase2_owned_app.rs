//! Dedicated inert native-action fixture. Never opens user documents or other apps.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    if !dir.join("phase2-fixture.marker").is_file() {
        return;
    }
    let value = serde_json::json!({"pid":std::process::id(),"configured_key_present":std::env::var_os("PHASE2_API_TEST_SECRET").is_some(),"dao_injection_present":std::env::var_os("DAO_PHASE2_SECRET").is_some(),"webview_injection_present":std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_some(),"cwd_matches_exe_parent":std::env::current_dir().ok().as_deref()==Some(dir)});
    if std::fs::write(dir.join("phase2-ready.json"), value.to_string()).is_err() {
        return;
    }
    if dir.join("exit-short.marker").is_file() {
        return;
    }
    std::thread::sleep(std::time::Duration::from_secs(30));
}
