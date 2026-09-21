use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    /// Full Chat Completions URL, e.g. https://api.openai.com/v1/chat/completions.
    pub endpoint: String,
    pub model: String,
    #[serde(default = "key_env")]
    pub api_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub label: String,
    pub endpoint: String,
    pub api_key_env: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelChoice {
    pub id: String,
    pub connection_id: String,
    pub name: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelCatalog {
    pub connections: Vec<Connection>,
    pub choices: Vec<ModelChoice>,
    pub active: Option<String>,
}
impl ModelCatalog {
    pub fn resolve(&self, id: &str) -> Result<ModelConfig> {
        let choice = self
            .choices
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| anyhow::anyhow!("请选择有效模型"))?;
        let connection = self
            .connections
            .iter()
            .find(|c| c.id == choice.connection_id)
            .ok_or_else(|| anyhow::anyhow!("模型所属连接不存在"))?;
        Ok(ModelConfig {
            endpoint: connection.endpoint.clone(),
            model: choice.name.clone(),
            api_key_env: connection.api_key_env.clone(),
        })
    }
    pub fn validate(&self) -> Result<()> {
        use std::collections::HashSet;
        ensure!(
            self.connections.len() <= 16 && self.choices.len() <= 64,
            "连接或模型数量过多"
        );
        let mut ids = HashSet::new();
        for c in &self.connections {
            ensure!(
                !c.id.is_empty() && c.id.len() <= 80 && ids.insert(&c.id),
                "连接 ID 重复或无效"
            );
            ensure!(
                !c.label.trim().is_empty()
                    && c.label.len() <= 120
                    && !c.label.chars().any(char::is_control),
                "连接名称无效"
            );
            ModelConfig {
                endpoint: c.endpoint.clone(),
                model: "validation".into(),
                api_key_env: c.api_key_env.clone(),
            }
            .validate()?;
        }
        let mut ids = HashSet::new();
        for m in &self.choices {
            ensure!(
                !m.id.is_empty() && m.id.len() <= 80 && ids.insert(&m.id),
                "模型 ID 重复或无效"
            );
            self.resolve(&m.id)?.validate()?;
        }
        if let Some(id) = &self.active {
            self.resolve(id)?;
        }
        Ok(())
    }
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
