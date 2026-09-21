//! UI-independent settings service. Reads never return credential values.
use crate::{
    config::Config,
    core::Cancellation,
    credentials,
    model::{
        self,
        provider::{Connection, ModelChoice, ModelConfig},
    },
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
    sync::Mutex,
};

static WRITER: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
pub struct SettingsView {
    pub revision: String,
    pub config: Config,
    pub credential_available: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyUpdate {
    pub connection_id: String,
    pub key: String,
    #[serde(default)]
    pub persist: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsUpdate {
    pub expected_revision: String,
    pub config: Config,
    #[serde(default)]
    pub keys: Vec<KeyUpdate>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionTest {
    pub model: ModelConfig,
    pub key: Option<String>,
}

fn revision(config: &Config) -> Result<String> {
    let mut hash = DefaultHasher::new();
    serde_json::to_vec(config)?.hash(&mut hash);
    Ok(format!("{:016x}", hash.finish()))
}

// Import CLI/legacy edits into the catalog without discarding saved connections.
fn import_legacy(config: &mut Config) {
    if let Some(model) = &config.model {
        if config
            .models
            .active
            .as_deref()
            .and_then(|id| config.models.resolve(id).ok())
            .as_ref()
            == Some(model)
        {
            return;
        }
        config.models.connections.retain(|c| c.id != "legacy");
        config.models.choices.retain(|m| m.id != "legacy");
        config.models.connections.push(Connection {
            id: "legacy".into(),
            label: "已有 CLI 连接".into(),
            endpoint: model.endpoint.clone(),
            api_key_env: model.api_key_env.clone(),
        });
        config.models.choices.push(ModelChoice {
            id: "legacy".into(),
            connection_id: "legacy".into(),
            name: model.model.clone(),
        });
        config.models.active = Some("legacy".into());
    } else {
        config.models.active = None;
    }
}

pub fn read(path: &Path) -> Result<SettingsView> {
    let mut config = Config::load(path)?;
    let revision = revision(&config)?;
    import_legacy(&mut config);
    let credential_available = config
        .models
        .connections
        .iter()
        .filter_map(|c| {
            let model = ModelConfig {
                endpoint: c.endpoint.clone(),
                model: "availability".into(),
                api_key_env: c.api_key_env.clone(),
            };
            credentials::resolve(&model)
                .ok()
                .flatten()
                .filter(|s| !s.is_empty())
                .map(|_| c.id.clone())
        })
        .collect();
    Ok(SettingsView {
        revision,
        config,
        credential_available,
    })
}

pub fn save(path: &Path, mut update: SettingsUpdate) -> Result<SettingsView> {
    let _writer = WRITER
        .lock()
        .map_err(|_| anyhow::anyhow!("配置服务不可用"))?;
    ensure!(
        revision(&Config::load(path)?)? == update.expected_revision,
        "配置已被其他入口修改，请重新载入后保存"
    );
    update.config.models.validate()?;
    update.config.scope(true)?;
    update.config.model = update
        .config
        .models
        .active
        .as_deref()
        .map(|id| update.config.models.resolve(id))
        .transpose()?;
    ensure!(
        serde_json::to_vec_pretty(&update.config)?.len() <= 64 * 1024,
        "配置文件过大"
    );
    ensure!(update.keys.len() <= 16, "密钥更新数量过多");
    let mut keys = Vec::new();
    for change in update.keys {
        let connection = update
            .config
            .models
            .connections
            .iter()
            .find(|c| c.id == change.connection_id)
            .ok_or_else(|| anyhow::anyhow!("密钥所属连接不存在"))?;
        let model = ModelConfig {
            endpoint: connection.endpoint.clone(),
            model: "credential-validation".into(),
            api_key_env: connection.api_key_env.clone(),
        };
        ensure!(
            !model.api_key_env.is_empty(),
            "保存密钥需要设置凭据引用名称"
        );
        let key = change.key.trim().to_owned();
        ensure!(!key.is_empty() && key.len() <= 2560, "密钥为空或过长");
        model::ModelClient::with_key(&model, Some(&key))?;
        keys.push((model, key, change.persist));
    }
    // Credential and JSON stores cannot share a transaction. Validate everything first;
    // a credential-store error must not report a successful configuration save.
    for (model, key, persist) in &keys {
        if *persist {
            credentials::save(model, key)?;
        }
    }
    update.config.save(path)?;
    for (model, key, _) in keys {
        credentials::set_session(&model, key);
    }
    read(path)
}

pub async fn test_connection(
    request: ConnectionTest,
    cancel: &Cancellation,
) -> Result<model::ConnectionCheck> {
    let client = match request.key.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(key) => model::ModelClient::with_key(&request.model, Some(key.trim()))?,
        None => model::ModelClient::new(&request.model)?,
    };
    model::check_client(&client, cancel).await
}
