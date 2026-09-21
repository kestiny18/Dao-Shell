fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "session_info",
            "perform",
            "confirm_open",
            "cancel",
            "get_settings",
            "save_settings",
            "test_connection",
            "computer_overview",
        ]),
    ))
    .expect("Tauri build configuration");
}
