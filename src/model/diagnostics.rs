use super::{ModelClient, provider::ModelConfig};
use crate::core::Cancellation;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Debug, serde::Serialize)]
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
