use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
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

impl ModelConfig {
    /// Validate without reading credentials or making a network request.
    pub fn validate(&self) -> Result<reqwest::Url> {
        ensure!(
            !self.endpoint.starts_with('\'') && !self.endpoint.ends_with('\''),
            "endpoint 首尾包含单引号；在 CMD 中请改用双引号，或运行 setup 直接输入地址"
        );
        ensure!(self.endpoint.len() <= 4096, "模型 endpoint 过长");
        ensure!(
            self.api_key_env.len() <= 128,
            "密钥环境变量名称过长；不要填写 API Key 本身"
        );
        ensure!(
            self.endpoint == self.endpoint.trim() && !self.endpoint.chars().any(char::is_control),
            "模型 endpoint 不能包含空白边界或控制字符"
        );
        let endpoint = reqwest::Url::parse(&self.endpoint)
            .map_err(|_| anyhow::anyhow!("模型 endpoint 必须为完整 URL"))?;
        ensure!(endpoint.host_str().is_some(), "模型 endpoint 缺少主机地址");
        ensure!(
            endpoint.path() != "/" && !endpoint.path().is_empty(),
            "endpoint 只有服务地址；请填写包含 /chat/completions 的完整请求地址"
        );
        ensure!(
            endpoint.scheme() == "https" || (endpoint.scheme() == "http" && is_loopback(&endpoint)),
            "远程模型必须使用 HTTPS；仅本机服务允许 HTTP"
        );
        ensure!(
            endpoint.username().is_empty()
                && endpoint.password().is_none()
                && endpoint.query().is_none()
                && endpoint.fragment().is_none(),
            "endpoint 不能包含凭据、查询参数或 fragment"
        );
        ensure!(
            !self.model.trim().is_empty()
                && self.model.len() <= 256
                && !self.model.chars().any(char::is_control),
            "模型名称不能为空、超过 256 字节或包含控制字符"
        );
        ensure!(
            self.api_key_env.is_empty()
                || self.api_key_env.bytes().enumerate().all(|(i, b)| {
                    b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                }),
            "密钥环境变量名称只能包含字母、数字和下划线，且不能以数字开头；不要填写 API Key 本身"
        );
        Ok(endpoint)
    }
}

pub(crate) fn is_loopback(endpoint: &reqwest::Url) -> bool {
    endpoint
        .host_str()
        .is_some_and(|h| matches!(h, "localhost" | "127.0.0.1" | "[::1]"))
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
