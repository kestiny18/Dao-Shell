//! Shared model transport and an isolated, synthetic connection check.
use crate::{
    config::{ModelConfig, is_loopback},
    core::Cancellation,
};
use anyhow::{Context, Result, bail, ensure};
use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, HeaderValue},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

pub(crate) const TOOL_LIMIT: usize = 12;

pub(crate) struct ModelClient {
    client: Client,
    endpoint: Url,
    model: String,
    key: Option<HeaderValue>,
}

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}
#[derive(Deserialize)]
struct Choice {
    message: Assistant,
}
#[derive(Deserialize)]
struct Usage {
    total_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Assistant {
    pub content: Option<String>,
    #[serde(default, deserialize_with = "nullable_calls")]
    pub tool_calls: Vec<ToolCall>,
    pub reasoning_content: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: Function,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Function {
    pub name: String,
    pub arguments: String,
}

fn nullable_calls<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<ToolCall>, D::Error> {
    Ok(Option::<Vec<ToolCall>>::deserialize(deserializer)?.unwrap_or_default())
}

impl Assistant {
    pub fn replay(&self) -> Value {
        let mut message = json!({"role":"assistant","content":self.content});
        if !self.tool_calls.is_empty() {
            message["tool_calls"] = json!(self.tool_calls);
        }
        // Some compatible reasoning providers require this on tool-result turns.
        if let Some(reasoning) = &self.reasoning_content {
            message["reasoning_content"] = json!(reasoning);
        }
        message
    }
}

impl ModelClient {
    pub fn new(config: &ModelConfig) -> Result<Self> {
        let key = crate::credentials::resolve(config)?;
        Self::with_key(config, key.as_deref())
    }

    pub(crate) fn with_key(config: &ModelConfig, key: Option<&str>) -> Result<Self> {
        let endpoint = config.validate()?;
        let key = key.map(authorization).transpose()?.flatten();
        ensure!(
            is_loopback(&endpoint) || key.is_some(),
            "未找到 API Key；运行 daosh setup 输入密钥，或设置环境变量 {}",
            config.api_key_env
        );
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            client,
            endpoint,
            model: config.model.clone(),
            key,
        })
    }

    pub async fn request(
        &self,
        messages: &[Value],
        tools: Value,
        cancel: &Cancellation,
    ) -> Result<(Assistant, Option<u64>)> {
        cancel.check()?;
        let payload = json!({"model":self.model,"messages":messages,"tools":tools,
            "tool_choice":"auto","parallel_tool_calls":false,"stream":false});
        let mut request = self.client.post(self.endpoint.clone()).json(&payload);
        if let Some(key) = &self.key {
            request = request.header(AUTHORIZATION, key.clone());
        }
        let network = async {
            // Never expose remote bodies, headers, or raw transport errors (which can contain secrets).
            let mut response = request.send().await.map_err(|e| {
                anyhow::anyhow!(if e.is_timeout() {
                    "模型请求超时；请检查网络或服务负载，已执行操作不会重试"
                } else if e.is_connect() {
                    "无法连接模型服务；请检查地址、端口、代理和服务是否启动"
                } else if e.is_builder() {
                    "无法构造模型请求；请检查密钥格式和本地配置"
                } else {
                    "模型连接失败；请检查网络与服务配置，已执行操作不会重试"
                })
            })?;
            let status = response.status();
            ensure!(
                status.is_success(),
                "模型服务返回 HTTP {}：{}",
                status.as_u16(),
                status_hint(status.as_u16())
            );
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow::anyhow!("模型响应读取失败；已执行操作不会重试"))?
            {
                ensure!(
                    body.len() + chunk.len() <= 512 * 1024,
                    "模型响应超过大小限制"
                );
                body.extend_from_slice(&chunk);
            }
            let response: Completion = serde_json::from_slice(&body)
                .map_err(|_| anyhow::anyhow!("模型未返回兼容的 Chat Completions 工具调用格式"))?;
            let message = response
                .choices
                .into_iter()
                .next()
                .context("模型返回空 choices")?
                .message;
            ensure!(
                message.tool_calls.len() <= TOOL_LIMIT,
                "模型单次工具调用超限，未执行这批调用"
            );
            let mut ids = HashSet::new();
            for call in &message.tool_calls {
                ensure!(
                    call.r#type == "function"
                        && !call.id.is_empty()
                        && call.id.len() <= 128
                        && ids.insert(&call.id)
                        && call.function.arguments.len() <= 16 * 1024,
                    "模型返回无效或重复的工具调用，未执行这批调用"
                );
            }
            Ok((message, response.usage.and_then(|u| u.total_tokens)))
        };
        let result = tokio::select! { result = network => result,
        _ = async { loop { if cancel.is_cancelled() { break; }
            tokio::time::sleep(Duration::from_millis(50)).await; } } => bail!("本轮已取消") };
        cancel.check()?;
        result
    }
}

fn authorization(raw: &str) -> Result<Option<HeaderValue>> {
    let key = raw.trim();
    if key.is_empty() {
        return Ok(None);
    }
    ensure!(
        key.bytes().all(|b| b.is_ascii_graphic()),
        "API Key 包含非法字符（内部空白、控制字符或非 ASCII 字符）；请重新复制密钥，首尾空白会自动去除"
    );
    let mut header = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| anyhow::anyhow!("API Key 无法构成有效请求头；请重新输入密钥"))?;
    header.set_sensitive(true);
    Ok(Some(header))
}

fn status_hint(status: u16) -> &'static str {
    match status {
        301..=399 => "地址发生重定向，未跟随；请填写最终的完整请求地址",
        400 | 422 => "请求格式或参数不兼容；请确认模型支持 Chat Completions 与工具调用",
        401 => "身份验证失败；请检查配置的密钥环境变量及密钥有效性",
        403 => "服务拒绝访问；请检查账户权限和模型访问权限",
        404 => "接口或模型不存在；endpoint 需要包含完整的 /chat/completions 路径",
        429 => "请求受限或额度不足；请检查服务配额，稍后再试",
        500..=599 => "模型服务暂时故障；请稍后再试",
        _ => "请求未成功；请检查服务兼容性与配置",
    }
}

#[derive(Debug)]
pub struct ConnectionCheck {
    pub elapsed_ms: u128,
    pub total_tokens: Option<u64>,
}

/// Two requests with fixed synthetic data. No Runtime, file access, or tool execution.
pub async fn check_connection(
    config: &ModelConfig,
    cancel: &Cancellation,
) -> Result<ConnectionCheck> {
    let client = ModelClient::new(config)?;
    check_client(&client, cancel).await
}

pub(crate) async fn check_client(
    client: &ModelClient,
    cancel: &Cancellation,
) -> Result<ConnectionCheck> {
    let started = Instant::now();
    let tools = json!([{"type":"function","function":{
        "name":"dao_shell_connection_check","description":"Return a fixed synthetic connection check result.",
        "parameters":{"type":"object","properties":{},"additionalProperties":false}
    }}]);
    let mut messages = vec![
        json!({"role":"system","content":"This is a synthetic connection check. First call dao_shell_connection_check with {}. After receiving its result, reply with only its reply field. Do not call any other tool."}),
        json!({"role":"user","content":"Run the connection check now."}),
    ];
    let (first, first_tokens) = client.request(&messages, tools.clone(), cancel).await?;
    ensure!(
        first.tool_calls.len() == 1,
        "模型可连接，但没有返回单个诊断工具调用；工具调用兼容性尚未通过"
    );
    let call = &first.tool_calls[0];
    ensure!(
        call.function.name == "dao_shell_connection_check"
            && serde_json::from_str::<Value>(&call.function.arguments).ok() == Some(json!({})),
        "模型可连接，但诊断工具名称或参数不符合要求；未执行任何工具"
    );
    messages.push(first.replay());
    messages.push(json!({"role":"tool","tool_call_id":call.id,
        "content":json!({"status":"ok","reply":"DAO_SHELL_OK"}).to_string()}));
    let (second, second_tokens) = client.request(&messages, tools, cancel).await?;
    ensure!(
        second.tool_calls.is_empty()
            && second.content.as_deref().map(str::trim) == Some("DAO_SHELL_OK"),
        "模型已返回工具调用，但未正确读回诊断结果；工具结果回传兼容性尚未通过"
    );
    Ok(ConnectionCheck {
        elapsed_ms: started.elapsed().as_millis(),
        total_tokens: first_tokens
            .zip(second_tokens)
            .and_then(|(a, b)| a.checked_add(b)),
    })
}
