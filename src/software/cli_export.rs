//! Fixed, read-only structured export fallback. Missing exported IDs are always unknown.
use crate::core::Cancellation;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
#[cfg(windows)]
pub fn installed(cancel: &Cancellation) -> Result<Value> {
    use std::os::windows::process::CommandExt;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("provider_unavailable"))?;
    let executable = root.join("Microsoft/WindowsApps/winget.exe");
    ensure!(
        executable.is_absolute() && executable.exists(),
        "provider_unavailable"
    );
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("installed.json");
    let mut child = Command::new(&executable)
        .args(["export", "--output"])
        .arg(&output)
        .args([
            "--include-versions",
            "--source",
            "winget",
            "--disable-interactivity",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;
    let began = Instant::now();
    loop {
        if cancel.is_cancelled() || began.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("provider_cancelled_or_timeout");
        }
        if let Some(status) = child.try_wait()? {
            ensure!(status.success(), "structured_export_unavailable");
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    ensure!(
        std::fs::metadata(&output)?.len() <= 2 * 1024 * 1024,
        "provider_budget"
    );
    let value: Value = serde_json::from_slice(&std::fs::read(output)?)?;
    parse(&value)
}
#[cfg(not(windows))]
pub fn installed(_: &Cancellation) -> Result<Value> {
    anyhow::bail!("unsupported_platform")
}
pub fn parse(value: &Value) -> Result<Value> {
    let sources = value["Sources"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("invalid_export"))?;
    let mut items = Vec::new();
    for source in sources {
        let s = &source["SourceDetails"];
        ensure!(
            s["Identifier"] == "Microsoft.Winget.Source_8wekyb3d8bbwe"
                && s["Name"] == "winget"
                && s["Type"] == "Microsoft.PreIndexed.Package",
            "source_identity_mismatch"
        );
        for package in source["Packages"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid_export"))?
        {
            let id = package["PackageIdentifier"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or_else(|| anyhow::anyhow!("invalid_export"))?;
            let version = package["Version"]
                .as_str()
                .filter(|s| !s.eq_ignore_ascii_case("unknown") && s.len() <= 128);
            items.push(json!({"id":id,"name":id,"source":"winget","source_id":"Microsoft.Winget.Source_8wekyb3d8bbwe","installed_version":version,"installation_state":"present_unscoped","scope":"unknown","supported_actions":[],"reason":"exact_scope_and_instance_unverified"}));
            ensure!(items.len() <= 4096, "provider_budget");
        }
    }
    Ok(
        json!({"items":items,"coverage":"partial","missing_means":"unknown","provider":"winget_structured_export","observed_at":crate::core::now(),"limits":"Export covers only source-associated IDs; never proves absence or exact scope/installation instance. It is not an uninstall credential."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_export_is_presence_only_and_rejects_foreign_source() {
        let mut v = json!({"Sources":[{"SourceDetails":{"Identifier":"Microsoft.Winget.Source_8wekyb3d8bbwe","Name":"winget","Type":"Microsoft.PreIndexed.Package"},"Packages":[{"PackageIdentifier":"Fixture.Owned","Version":"Unknown"}]}]});
        let parsed = parse(&v).unwrap();
        assert_eq!(parsed["coverage"], "partial");
        assert_eq!(parsed["missing_means"], "unknown");
        assert!(parsed["items"][0]["installed_version"].is_null());
        assert_eq!(parsed["items"][0]["scope"], "unknown");
        v["Sources"][0]["SourceDetails"]["Identifier"] = json!("foreign");
        assert!(parse(&v).is_err());
    }
}
