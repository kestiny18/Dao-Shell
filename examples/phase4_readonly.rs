fn main() {
    let cancel = dao_shell::core::Cancellation::default();
    let native = dao_shell::software::probe();
    let fallback = dao_shell::software::cli_export::installed(&cancel);
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"native":native,"export":fallback.map(|v|serde_json::json!({"available":true,"items_count":v["items"].as_array().map(Vec::len),"coverage":v["coverage"],"missing_means":v["missing_means"]})).unwrap_or_else(|_|serde_json::json!({"available":false}))})).unwrap());
}
