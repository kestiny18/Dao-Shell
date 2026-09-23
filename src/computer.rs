//! Local observations only. Opening the home page never contacts a model.
use crate::{
    core::{Cancellation, now},
    resources,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::time::Instant;
use sysinfo::{Networks, System};

#[cfg(windows)]
mod icons;

pub fn overview(cancel: &Cancellation) -> Result<Value> {
    cancel.check()?;
    let mut networks = Networks::new_with_refreshed_list();
    let started = Instant::now();
    let mut facts = resources::snapshot(
        &resources::Request {
            sample_ms: Some(700),
            limit: Some(5),
            ..Default::default()
        },
        cancel,
        false,
    )?;
    networks.refresh(true);
    let elapsed = started.elapsed().as_secs_f64();
    let adapters: Vec<_> = networks
        .iter()
        .map(|(name, data)| {
            json!({
                "name":name, "has_ip":!data.ip_networks().is_empty(),
                "received_bytes_per_second":data.received() as f64 / elapsed,
                "transmitted_bytes_per_second":data.transmitted() as f64 / elapsed
            })
        })
        .collect();
    facts["device"] = json!({"os":System::long_os_version(), "host":System::host_name(), "architecture":System::cpu_arch(), "uptime_seconds":System::uptime()});
    facts["network"] = json!({"adapters":adapters,"observed_at":now(),"limits":"接口计数器的短时采样；已配置 IP 不代表互联网可用，不主动发起联网探测。"});
    cancel.check()?;
    facts["applications"] = applications(cancel)?;
    cancel.check()?;
    Ok(facts)
}

#[cfg(not(windows))]
fn applications(_: &Cancellation) -> Result<Value> {
    Ok(
        json!({"items":[],"available":false,"limits":"当前平台尚未实现应用清单。","observed_at":now()}),
    )
}

#[cfg(windows)]
fn applications(cancel: &Cancellation) -> Result<Value> {
    use std::collections::BTreeSet;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
        System::Registry::*,
    };
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn value(key: HKEY, name: &str) -> Option<String> {
        let mut buffer = vec![0u16; 2048];
        let mut size = (buffer.len() * 2) as u32;
        let name = wide(name);
        // Registry data is bounded and treated as display text, never executable input.
        let status = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let length = ((size as usize) / 2).min(buffer.len());
        let text = String::from_utf16_lossy(&buffer[..length])
            .trim_end_matches('\0')
            .to_owned();
        (!text.trim().is_empty()).then_some(text)
    }
    let mut items = Vec::new();
    let mut icon_sources = std::collections::HashMap::new();
    let mut seen = BTreeSet::new();
    let mut partial = false;
    let mut readable_views = 0;
    let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall");
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let mut raw = std::ptr::null_mut();
            let result =
                unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ | view, &mut raw) };
            if result == ERROR_FILE_NOT_FOUND {
                continue;
            }
            if result != ERROR_SUCCESS {
                partial = true;
                continue;
            }
            let key = Key(raw);
            readable_views += 1;
            for index in 0..4096 {
                cancel.check()?;
                let mut name = vec![0u16; 256];
                let mut len = name.len() as u32;
                let result = unsafe {
                    RegEnumKeyExW(
                        key.0,
                        index,
                        name.as_mut_ptr(),
                        &mut len,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if result == ERROR_NO_MORE_ITEMS {
                    break;
                }
                if result != ERROR_SUCCESS {
                    partial = true;
                    continue;
                }
                let mut child = std::ptr::null_mut();
                if unsafe { RegOpenKeyExW(key.0, name.as_ptr(), 0, KEY_READ | view, &mut child) }
                    != ERROR_SUCCESS
                {
                    partial = true;
                    continue;
                }
                let child = Key(child);
                if let Some(name) = value(child.0, "DisplayName") {
                    let version = value(child.0, "DisplayVersion");
                    let publisher = value(child.0, "Publisher");
                    if seen.insert((name.clone(), version.clone(), publisher.clone())) {
                        if let Some(source) = value(child.0, "DisplayIcon") {
                            icon_sources.insert(items.len(), source);
                        }
                        items.push(json!({"name":name,"version":version,"publisher":publisher}));
                    }
                }
                if index == 4095 {
                    partial = true;
                }
            }
        }
    }
    // A bounded best-effort pass: registry icon paths never become UI file URLs.
    // Missing/slow sources simply leave the name-based fallback visible.
    let icon_started = Instant::now();
    let mut cache = std::collections::HashMap::new();
    for (index, item) in items.iter_mut().enumerate().take(128) {
        cancel.check()?;
        if icon_started.elapsed() > std::time::Duration::from_secs(2) {
            break;
        }
        if let Some(source) = icon_sources.get(&index) {
            let icon = cache
                .entry(source.clone())
                .or_insert_with(|| icons::load(source));
            if let Some(icon) = icon {
                item["icon"] = json!(icon);
            }
        }
    }
    items.sort_by_key(|item| item["name"].as_str().unwrap_or("").to_lowercase());
    Ok(
        json!({"items":items,"available":readable_views > 0,"partial":partial,"observed_at":now(),"limits":"仅枚举当前用户和本机 32/64 位卸载注册表记录；可能含组件，不完整覆盖商店应用或便携应用。"}),
    )
}
