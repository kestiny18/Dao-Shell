fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "session_info",
            "perform",
            "confirm_open",
            "cancel",
        ]),
    ))
    .expect("Tauri build configuration");
}
