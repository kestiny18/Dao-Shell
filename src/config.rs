use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub use crate::model::provider::ModelConfig;
pub(crate) use crate::model::provider::is_loopback;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    #[default]
    Restricted,
    Full,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub access_mode: AccessMode,
    #[serde(default)]
    pub models: crate::model::provider::ModelCatalog,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub write_roots: Vec<PathBuf>,
    pub model: Option<ModelConfig>,
}
impl Config {
    pub fn scope(&self, writable: bool) -> Result<crate::files::Scope> {
        let roots: Vec<_> = self
            .read_roots
            .iter()
            .chain(&self.write_roots)
            .cloned()
            .collect();
        if self.access_mode == AccessMode::Full {
            let defaults = if roots.is_empty() {
                [
                    dirs::download_dir(),
                    dirs::document_dir(),
                    dirs::desktop_dir(),
                ]
                .into_iter()
                .flatten()
                .filter(|p| p.is_dir())
                .collect()
            } else {
                roots
            };
            crate::files::Scope::full(&defaults, writable)
        } else {
            crate::files::Scope::new(&roots, if writable { &self.write_roots } else { &[] })
        }
    }
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).context("无法读取配置")?;
        ensure!(bytes.len() <= 64 * 1024, "配置文件过大");
        serde_json::from_slice(&bytes).context("配置格式错误")
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(bytes.len() <= 64 * 1024, "配置文件过大，原配置保留");
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut pending =
            tempfile::NamedTempFile::new_in(parent).context("无法准备配置文件，原配置保留")?;
        pending
            .write_all(&bytes)
            .context("无法写入配置，原配置保留")?;
        pending
            .as_file()
            .sync_all()
            .context("无法同步配置，原配置保留")?;
        pending.persist(path).context("无法保存配置，原配置保留")?;
        Ok(())
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
