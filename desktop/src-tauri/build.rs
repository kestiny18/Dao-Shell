fn main() {
    prepare_index_payload();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "inventory_command",
            "index_service_command",
            "language_info",
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
            "process_action",
        ]),
    ))
    .expect("Tauri build configuration");
}
fn prepare_index_payload() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let directory = root.join("index-payload");
    let manifest = directory.join("manifest.json");
    let source = directory.join("dao-index-worker.exe");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-env-changed=DAO_SHELL_REQUIRE_INDEX_PAYLOAD");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let generated = if manifest.exists() {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest).expect("read payload manifest"))
                .expect("parse payload manifest");
        assert_eq!(value["schema"], 1, "unsupported payload manifest");
        let bytes = std::fs::read(&source).expect("read prepared worker payload");
        assert!(bytes.len() <= 64 * 1024 * 1024, "worker payload too large");
        let digest = format!("{:X}", Sha256::digest(&bytes));
        assert_eq!(value["sha256"], digest, "prepared worker digest mismatch");
        assert_eq!(
            value["bytes"],
            bytes.len(),
            "prepared worker length mismatch"
        );
        let embedded = out.join("dao-index-worker.exe");
        std::fs::write(&embedded, bytes).expect("freeze payload");
        format!(
            "pub const PAYLOAD: Option<(&[u8], &str)> = Some((include_bytes!({:?}), {:?}));",
            embedded, digest
        )
    } else {
        assert!(
            std::env::var_os("DAO_SHELL_REQUIRE_INDEX_PAYLOAD").is_none(),
            "run scripts/prepare-index-payload.ps1 before packaging"
        );
        "pub const PAYLOAD: Option<(&[u8], &str)> = None;".into()
    };
    std::fs::write(out.join("index_payload.rs"), generated).expect("write payload binding");
}
