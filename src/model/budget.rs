//! Local process-run token guard; provider billing remains authoritative.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budget {
    pub output_tokens: u32,
    pub run_tokens: u64,
    pub output_parameter: OutputParameter,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputParameter {
    #[default]
    Auto,
    MaxTokens,
    MaxCompletionTokens,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            output_tokens: 2048,
            run_tokens: 1_000_000,
            output_parameter: OutputParameter::Auto,
        }
    }
}
impl Budget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=32768).contains(&self.output_tokens)
                && self.run_tokens >= u64::from(self.output_tokens)
                && self.run_tokens <= 1_000_000_000,
            "invalid_model_budget"
        );
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Serialize)]
pub struct Usage {
    pub actual_tokens: u64,
    pub uncertain_reserved_tokens: u64,
    pub requests: u64,
}
static USAGE: Mutex<Usage> = Mutex::new(Usage {
    actual_tokens: 0,
    uncertain_reserved_tokens: 0,
    requests: 0,
});
pub fn snapshot() -> Usage {
    *USAGE.lock().unwrap_or_else(|e| e.into_inner())
}
pub(crate) fn reserve(config: &Budget, input_bytes: usize) -> Result<u64> {
    config.validate()?;
    let amount = (input_bytes as u64).saturating_add(u64::from(config.output_tokens));
    let mut used = USAGE.lock().unwrap_or_else(|e| e.into_inner());
    ensure!(
        used.actual_tokens
            .saturating_add(used.uncertain_reserved_tokens)
            .saturating_add(amount)
            <= config.run_tokens,
        "model_budget_exhausted: 本次运行模型预算不足，请查看用量或调整本地预算；请求尚未发送"
    );
    used.uncertain_reserved_tokens = used.uncertain_reserved_tokens.saturating_add(amount);
    used.requests += 1;
    Ok(amount)
}
pub(crate) fn settle(reserved: u64, actual: Option<u64>) {
    if let Some(actual) = actual.filter(|v| *v > 0) {
        let mut used = USAGE.lock().unwrap_or_else(|e| e.into_inner());
        used.uncertain_reserved_tokens = used.uncertain_reserved_tokens.saturating_sub(reserved);
        used.actual_tokens = used.actual_tokens.saturating_add(actual);
    }
}
