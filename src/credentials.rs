//! Keys never enter Config or operation records. Saved keys are endpoint-bound.
use crate::config::ModelConfig;
use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

static SESSION: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn target(model: &ModelConfig) -> String {
    // Windows credential names are case-insensitive; hex preserves URL path case.
    let identity = format!("{}\n{}", model.endpoint, model.api_key_env);
    let hex: String = identity.bytes().map(|b| format!("{b:02x}")).collect();
    format!("Dao-Shell/model/{hex}")
}

pub(crate) fn set_session(model: &ModelConfig, key: String) {
    SESSION
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .insert(target(model), key);
}

pub(crate) fn resolve(model: &ModelConfig) -> Result<Option<String>> {
    model.validate()?;
    if model.api_key_env.is_empty() {
        return Ok(None);
    }
    if let Some(key) = SESSION
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .get(&target(model))
    {
        return Ok(Some(key.clone()));
    }
    match std::env::var(&model.api_key_env) {
        Ok(key) if !key.trim().is_empty() => return Ok(Some(key)),
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(_) => bail!("API Key 环境变量不是有效文本；请重新输入密钥"),
    }
    stored::read(&target(model))
}

pub(crate) fn save(model: &ModelConfig, key: &str) -> Result<()> {
    model.validate()?;
    stored::write(&target(model), key)
}

#[cfg(not(windows))]
mod stored {
    use anyhow::{Result, bail};
    pub fn read(_: &str) -> Result<Option<String>> {
        Ok(None)
    }
    pub fn write(_: &str, _: &str) -> Result<()> {
        bail!("此平台暂不支持安全保存密钥；可仅本次使用或使用环境变量")
    }
}

#[cfg(windows)]
mod stored {
    use anyhow::{Context, Result, ensure};
    use windows_sys::Win32::{
        Foundation::{ERROR_NOT_FOUND, GetLastError},
        Security::Credentials::*,
    };

    pub fn read(target: &str) -> Result<Option<String>> {
        let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        let mut credential = std::ptr::null_mut();
        // CredRead allocates the credential and its blob until CredFree.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0 {
                let code = GetLastError();
                if code == ERROR_NOT_FOUND {
                    return Ok(None);
                }
                anyhow::bail!("无法读取 Windows 凭据（错误 {code}）；可在 setup 中重新输入密钥");
            }
            let value = &*credential;
            let bytes = if value.CredentialBlobSize == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize)
                    .to_vec()
            };
            CredFree(credential.cast());
            Ok(Some(
                String::from_utf8(bytes).context("保存的密钥格式无效；请重新输入")?,
            ))
        }
    }

    pub fn write(target: &str, key: &str) -> Result<()> {
        ensure!(
            key.len() <= 2560,
            "密钥超过 Windows 凭据长度限制；可仅本次使用"
        );
        let mut target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        let mut bytes = key.as_bytes().to_vec();
        let value = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // All pointers remain valid through the synchronous call.
        ensure!(
            unsafe { CredWriteW(&value, 0) } != 0,
            "无法保存 Windows 凭据；密钥仍可仅本次使用"
        );
        Ok(())
    }

    #[cfg(test)]
    pub fn delete(target: &str) {
        let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
        unsafe {
            CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> ModelConfig {
        ModelConfig {
            endpoint: format!(
                "https://example.invalid/{}/chat/completions",
                uuid::Uuid::new_v4()
            ),
            model: "fixture".into(),
            api_key_env: "DAO_TEST_CREDENTIAL".into(),
        }
    }

    #[test]
    fn session_keys_are_bound_to_endpoint_and_never_serialized() {
        let model = fixture();
        set_session(&model, "fixture-only-key".into());
        assert_eq!(
            resolve(&model).unwrap().as_deref(),
            Some("fixture-only-key")
        );
        let mut other = model.clone();
        other.endpoint = other.endpoint.replace("/chat/", "/Chat/");
        assert_ne!(target(&model).to_lowercase(), target(&other).to_lowercase());
        assert!(resolve(&other).unwrap().is_none());
        assert!(
            !serde_json::to_string(&model)
                .unwrap()
                .contains("fixture-only-key")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_round_trip_uses_an_isolated_target() {
        let model = fixture();
        let id = target(&model);
        save(&model, "disposable-test-key").unwrap();
        let read = stored::read(&id);
        stored::delete(&id);
        assert_eq!(read.unwrap().as_deref(), Some("disposable-test-key"));
        assert!(stored::read(&id).unwrap().is_none());
    }
}
