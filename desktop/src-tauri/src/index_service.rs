//! Explicit settings actions own elevation; ordinary search never calls this launcher.
use anyhow::{Result, ensure};
use dao_shell::{core::Cancellation, search_index::IndexClient};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
mod bundled {
    include!(concat!(env!("OUT_DIR"), "/index_payload.rs"));
}
pub struct Integration {
    path: PathBuf,
    management: Mutex<()>,
    busy: AtomicBool,
}
impl Integration {
    pub fn rebuild(&self, key: &str, confirmed: bool) -> Result<Value> {
        ensure!(confirmed, "请在本地设置中确认重建此卷的索引");
        #[cfg(windows)]
        {
            ensure!(
                dao_shell::file_index::worker::service::valid_volume_key(key),
                "无效卷标识"
            );
            let _gate = self
                .management
                .try_lock()
                .map_err(|_| anyhow::anyhow!("已有服务操作进行中"))?;
            let (_, sha) = bundled::PAYLOAD.ok_or_else(|| anyhow::anyhow!("安装产物尚未准备"))?;
            dao_shell::search_index::WindowsServiceIndexClient::new(sha.into())
                .rebuild(key, &Cancellation::default())
        }
        #[cfg(not(windows))]
        {
            let _ = key;
            anyhow::bail!("当前仅支持 Windows")
        }
    }
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            management: Mutex::new(()),
            busy: AtomicBool::new(false),
        }
    }
    fn enabled(&self) -> bool {
        dao_shell::config::Config::load(&self.path)
            .is_ok_and(|config| config.access_mode == dao_shell::config::AccessMode::Full)
    }
    pub fn client(&self, mode: dao_shell::config::AccessMode) -> Option<Arc<dyn IndexClient>> {
        if mode != dao_shell::config::AccessMode::Full {
            return None;
        }
        #[cfg(windows)]
        {
            bundled::PAYLOAD.map(|(_, sha)| {
                Arc::new(dao_shell::search_index::WindowsServiceIndexClient::new(
                    sha.into(),
                )) as Arc<dyn IndexClient>
            })
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
    pub fn status(&self) -> Result<Value> {
        #[cfg(windows)]
        let mut value = if let Some((_, sha)) = bundled::PAYLOAD {
            dao_shell::search_index::WindowsServiceIndexClient::new(sha.into()).status(&Cancellation::default()).unwrap_or_else(|_|json!({"state":"unavailable","message":"服务连接或身份核验失败；请检查状态，不会自动请求管理员权限。"}))
        } else {
            let status = dao_shell::file_index::worker::service::status()?;
            json!({"state":if status["state"]=="not_installed"{"not_installed"}else{"payload_missing"}})
        };
        #[cfg(not(windows))]
        let mut value = json!({"state":"unsupported_platform"});
        value["enabled"] = json!(self.enabled());
        value["payload_ready"] = json!(bundled::PAYLOAD.is_some());
        value["busy"] = json!(self.busy.load(Ordering::Acquire));
        Ok(value)
    }
    pub fn diagnostic(&self) -> Result<Value> {
        let status = self.status()?;
        Ok(safe_diagnostic(
            &status,
            bundled::PAYLOAD.map(|(_, sha)| sha),
        ))
    }
    pub fn command(&self, action: &str, confirmed: bool, delete_index: bool) -> Result<Value> {
        if action == "diagnostic" {
            return self.diagnostic();
        }
        if action == "status" {
            return self.status();
        }
        ensure!(confirmed, "请在本地设置中确认服务操作");
        ensure!(
            !delete_index || action == "uninstall",
            "删除索引只适用于卸载"
        );
        let _gate = self
            .management
            .try_lock()
            .map_err(|_| anyhow::anyhow!("已有服务操作进行中"))?;
        ensure!(
            matches!(
                action,
                "install"
                    | "upgrade"
                    | "repair"
                    | "rollback"
                    | "service-start"
                    | "service-stop"
                    | "uninstall"
            ),
            "不支持的服务操作"
        );
        let (bytes, sha) = bundled::PAYLOAD
            .ok_or_else(|| anyhow::anyhow!("安装产物尚未准备；请使用包含固定索引程序的构建"))?;
        #[cfg(windows)]
        {
            self.busy.store(true, Ordering::Release);
            struct Reset<'a>(&'a AtomicBool);
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::Release);
                }
            }
            let _reset = Reset(&self.busy);
            let outcome =
                dao_shell::search_index::install::launch(bytes, sha, action, delete_index)?;
            Ok(
                json!({"state":outcome["state"],"operation":outcome,"enabled":self.enabled(),"payload_ready":true}),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (bytes, sha);
            anyhow::bail!("当前仅支持 Windows 索引服务")
        }
    }
}

fn safe_code(value: &Value, allowed: &[&str]) -> Value {
    value
        .as_str()
        .filter(|code| allowed.contains(code))
        .map_or(Value::Null, |code| json!(code))
}
fn safe_diagnostic(status: &Value, expected: Option<&str>) -> Value {
    let digest =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    let states = [
        "not_installed",
        "running",
        "stopped",
        "unavailable",
        "version_mismatch",
        "payload_missing",
        "unsupported_platform",
    ];
    let phases = [
        "building",
        "catching_up",
        "ready",
        "coverage_incomplete",
        "retrying",
        "rebuild_required",
        "error",
        "initializing",
        "unavailable",
    ];
    let codes = [
        "rebuild_required",
        "index_not_ready",
        "cancelled",
        "stale_cursor",
        "budget_exhausted",
        "request_failed",
        "maintenance_stopped",
        "storage_identity_changed",
        "metadata_changed_retrying",
        "metadata_changed_retry_exhausted",
        "access_denied",
        "sharing_violation",
        "delete_pending",
        "unresolved_parent_chain",
        "metadata_unavailable",
        "metadata_access_or_sharing_denied",
        "query_unavailable",
        "journal_unavailable",
        "worker_unavailable",
        "worker_panicked",
        "worker_stopped",
        "unsupported_filesystem",
        "offline",
        "volume_unavailable",
        "volume_worker_limit",
        "not_fixed_volume",
    ];
    let stages = [
        "metadata_standard",
        "metadata_identity",
        "metadata_open",
        "file_usn_parse",
        "identity_check",
        "journal",
        "usn",
        "enumeration",
        "index_maintenance",
    ];
    let volumes:Vec<_>=status["volumes"].as_array().into_iter().flatten().take(128).enumerate().map(|(index,volume)|{
        let drive=volume["mounts"].as_array().into_iter().flatten().filter_map(Value::as_str).find(|value| {
            let bytes=value.as_bytes(); bytes.len()==3 && bytes[0].is_ascii_alphabetic() && bytes[1]==b':' && bytes[2]==b'\\'
        });
        let state=&volume["index_status"];
        let reasons:Vec<_>=state["coverage_reasons"].as_array().into_iter().flatten().map(|value|safe_code(value,&codes)).filter(|value|!value.is_null()).take(32).collect();
        json!({"volume":drive.map(str::to_owned).unwrap_or_else(||format!("volume-{}",index+1)),"phase":safe_code(&state["phase"],&phases),"generation":state["generation"].as_u64(),"updated_unix_ms":state["updated_unix_ms"].as_u64(),"complete":state["complete"].as_bool(),"error":safe_code(&state["error"],&codes),"worker_error":safe_code(&volume["worker_error"],&codes),"reason":safe_code(&volume["reason"],&codes),"coverage_reasons":reasons,"diagnostic":{"available":state["diagnostic"].is_object(),"stage":safe_code(&state["diagnostic"]["stage"],&stages),"code":safe_code(&state["diagnostic"]["code"],&codes),"native_code":state["diagnostic"]["native_code"].as_i64().filter(|code|(0..=65535).contains(code))}})
    }).collect();
    json!({"report_version":1,"product_version":env!("CARGO_PKG_VERSION"),"client_protocol":1,"expected_service_build":expected.filter(|value|digest(value)),"actual_service_build":status["build"].as_str().filter(|value|digest(value)),"state":safe_code(&status["state"],&states),"full_access":status["enabled"].as_bool().unwrap_or(false),"volumes":volumes,"note":"Only safe status fields are exported. Missing diagnostics require a newer matching service; no failure cause is inferred."})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_export_omits_paths_identity_and_raw_errors() {
        let status = json!({"enabled":true,"state":"version_mismatch","build":"A".repeat(64),"sid":"SECRET_SID","volumes":[{"mounts":["C:\\","C:\\SECRET_PATH"],"guid":"SECRET_GUID","worker_error":"SECRET_RAW","index_status":{"phase":"error","generation":4,"error":"request_failed","diagnostic":{"stage":"metadata_standard","code":"request_failed","native_code":5,"raw":"SECRET_RAW"}}},{"mounts":["SECRET_PATH"],"index_status":{"phase":"error"}}]});
        let report = safe_diagnostic(&status, Some(&"B".repeat(64)));
        let text = report.to_string();
        assert!(!text.contains("SECRET"));
        assert_eq!(report["volumes"][0]["volume"], "C:\\");
        assert_eq!(report["volumes"][0]["diagnostic"]["native_code"], 5);
        assert_eq!(report["volumes"][1]["diagnostic"]["available"], false);
        assert_eq!(report["state"], "version_mismatch");
    }
    #[test]
    fn management_requires_confirmation_and_never_accepts_arbitrary_commands() {
        let fixture = tempfile::tempdir().unwrap();
        let integration = Integration::new(fixture.path().join("config.json"));
        assert!(integration.command("enable", false, false).is_err());
        assert!(integration.command("run shell", true, false).is_err());
        assert!(integration.command("service-stop", true, true).is_err());
        assert!(integration.rebuild("not-a-key", false).is_err());
        assert!(integration.rebuild("../../path", true).is_err());
        assert!(!integration.path.exists());
    }
    #[test]
    fn full_access_ignores_legacy_preference_and_restricted_stays_scoped() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("config.json");
        std::fs::write(
            fixture.path().join("file-index-service.json"),
            br#"{"enabled":false}"#,
        )
        .unwrap();
        let mut config = dao_shell::config::Config::default();
        config.save(&path).unwrap();
        let integration = Integration::new(path.clone());
        assert!(!integration.enabled());
        config.access_mode = dao_shell::config::AccessMode::Full;
        config.save(&path).unwrap();
        assert!(integration.enabled());
        config.access_mode = dao_shell::config::AccessMode::Restricted;
        config.save(&path).unwrap();
        assert!(!integration.enabled());
        assert!(integration.command("enable", true, false).is_err());
        assert!(integration.command("disable", true, false).is_err());
        assert_eq!(
            std::fs::read(fixture.path().join("file-index-service.json")).unwrap(),
            br#"{"enabled":false}"#
        );
    }
}
