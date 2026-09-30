fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "session_info",
            "load_workspace",
            "save_workspace",
            "register_session",
            "finish_close",
            "choose_directory",
            "perform",
            "confirm_open",
            "cancel",
            "get_settings",
            "save_settings",
            "test_connection",
            "computer_overview",
            "computer_profile",
            "application_action",
        ]),
    ))
    .expect("Tauri build configuration");
}
