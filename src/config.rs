use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    /// Full Chat Completions URL, e.g. https://api.openai.com/v1/chat/completions.
    pub endpoint: String,
    pub model: String,
    #[serde(default = "key_env")]
    pub api_key_env: String,
}
fn key_env() -> String {
    "DAO_SHELL_API_KEY".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub write_roots: Vec<PathBuf>,
    pub model: Option<ModelConfig>,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).context("无法读取配置")?;
        ensure!(bytes.len() <= 64 * 1024, "配置文件过大");
        serde_json::from_slice(&bytes).context("配置格式错误")
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?).context("无法保存配置")
    }
    pub fn path() -> PathBuf {
        data_dir().join("config.json")
    }
}
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Dao-Shell")
}
