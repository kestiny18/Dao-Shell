//! Fixed read-only native collectors. No executable text or counter path is accepted from callers.
use serde_json::{Value, json};
#[cfg(windows)]
pub fn power() -> Value {
    use windows_sys::Win32::System::Power::*;
    let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
        return json!({"reason":"power_unavailable"});
    }
    let absent = s.BatteryFlag != 255 && s.BatteryFlag & 128 != 0;
    json!({"ac_online":match s.ACLineStatus{0=>Some(false),1=>Some(true),_=>None},"battery_present":if s.BatteryFlag==255{None}else{Some(!absent)},"percent":if absent||s.BatteryLifePercent==255{None}else{Some(s.BatteryLifePercent)},"estimated_remaining_seconds":if absent||s.BatteryLifeTime==u32::MAX{None}else{Some(s.BatteryLifeTime)},"reason":if absent{Some("battery_not_applicable")}else{None}})
}
#[cfg(not(windows))]
pub fn power() -> Value {
    json!({"reason":"unsupported_platform"})
}
#[cfg(windows)]
fn native_io(pid: u32) -> Option<(u64, (u64, u64))> {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return None;
    }
    let mut counters: IO_COUNTERS = unsafe { std::mem::zeroed() };
    let mut c = unsafe { std::mem::zeroed() };
    let mut e = c;
    let mut k = c;
    let mut u = c;
    let valid = unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) } != 0;
    let created = ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64;
    let ok = valid && unsafe { GetProcessIoCounters(h, &mut counters) } != 0;
    unsafe { CloseHandle(h) };
    ok.then_some((
        created,
        (counters.ReadTransferCount, counters.WriteTransferCount),
    ))
}
#[cfg(windows)]
pub fn io(pid: u32, started: u64) -> Option<(u64, u64)> {
    native_io(pid)
        .filter(|(c, _)| {
            c.checked_div(10_000_000)
                .and_then(|c| c.checked_sub(11_644_473_600))
                == Some(started)
        })
        .map(|(_, v)| v)
}
#[cfg(windows)]
pub fn io_exact(pid: u32, created: &str) -> Option<(u64, u64)> {
    let expected = created.parse::<u64>().ok()?;
    native_io(pid)
        .filter(|(c, _)| *c == expected)
        .map(|(_, v)| v)
}
#[cfg(not(windows))]
pub fn io(_: u32, _: u64) -> Option<(u64, u64)> {
    None
}
#[cfg(not(windows))]
pub fn io_exact(_: u32, _: &str) -> Option<(u64, u64)> {
    None
}
#[cfg(windows)]
pub struct Gpu {
    query: windows_sys::Win32::System::Performance::PDH_HQUERY,
    counters: Vec<(
        windows_sys::Win32::System::Performance::PDH_HCOUNTER,
        &'static str,
    )>,
    names: Vec<String>,
}
#[cfg(windows)]
impl Gpu {
    pub fn new(enabled: bool) -> Self {
        use windows_sys::Win32::System::Performance::*;
        let mut s = Self {
            query: std::ptr::null_mut(),
            counters: Vec::new(),
            names: Vec::new(),
        };
        if !enabled {
            return s;
        }
        unsafe {
            if PdhOpenQueryW(std::ptr::null(), 0, &mut s.query) != 0 {
                return s;
            }
            for (path, unit) in [
                (r"\GPU Engine(*)\Utilization Percentage", "percent"),
                (r"\GPU Adapter Memory(*)\Dedicated Usage", "dedicated_bytes"),
                (r"\GPU Adapter Memory(*)\Shared Usage", "shared_bytes"),
            ] {
                let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
                let mut c = std::ptr::null_mut();
                if PdhAddEnglishCounterW(s.query, path.as_ptr(), 0, &mut c) == 0 {
                    s.counters.push((c, unit));
                }
            }
            PdhCollectQueryData(s.query);
        }
        s
    }
    pub fn sample(&mut self) -> Vec<(String, &'static str, f64)> {
        use windows_sys::Win32::System::Performance::*;
        let mut rows = Vec::new();
        if self.query.is_null() {
            return rows;
        }
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return rows;
            }
            for (c, unit) in &self.counters {
                let mut bytes = 0;
                let mut count = 0;
                PdhGetFormattedCounterArrayW(
                    *c,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    std::ptr::null_mut(),
                );
                if bytes == 0 || bytes > 1024 * 1024 {
                    continue;
                }
                let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
                let ptr = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
                if PdhGetFormattedCounterArrayW(*c, PDH_FMT_DOUBLE, &mut bytes, &mut count, ptr)
                    != 0
                {
                    continue;
                }
                let max = (bytes as usize / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>())
                    .min(count as usize);
                for row in std::slice::from_raw_parts(ptr, max) {
                    if row.FmtValue.CStatus > 1 || row.szName.is_null() {
                        continue;
                    }
                    let mut len = 0;
                    while len < 1024 && *row.szName.add(len) != 0 {
                        len += 1;
                    }
                    if len == 1024 {
                        continue;
                    }
                    let name = format!(
                        "{}:{}",
                        String::from_utf16_lossy(std::slice::from_raw_parts(row.szName, len)),
                        unit
                    );
                    let value = row.FmtValue.Anonymous.doubleValue;
                    if value.is_finite() && value >= 0. {
                        rows.push((name, *unit, value));
                    }
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        if self.names.is_empty() {
            // Prefer memory plus bounded engine instances; identities remain frozen for this observation.
            self.names = rows
                .iter()
                .filter(|r| r.1 != "percent")
                .take(16)
                .chain(rows.iter().filter(|r| r.1 == "percent").take(32))
                .map(|r| r.0.clone())
                .collect();
        }
        self.names
            .iter()
            .filter_map(|name| rows.iter().find(|r| &r.0 == name).cloned())
            .collect()
    }
}
#[cfg(windows)]
impl Drop for Gpu {
    fn drop(&mut self) {
        if !self.query.is_null() {
            unsafe {
                windows_sys::Win32::System::Performance::PdhCloseQuery(self.query);
            }
        }
    }
}
#[cfg(not(windows))]
pub struct Gpu;
#[cfg(not(windows))]
impl Gpu {
    pub fn new(_: bool) -> Self {
        Self
    }
    pub fn sample(&mut self) -> Vec<(String, &'static str, f64)> {
        Vec::new()
    }
}
/// Bounded fixed OS queries; the script is selected by code, never model/user text.
#[cfg(windows)]
fn fixed_query(kind: &str, cancel: &crate::core::Cancellation) -> anyhow::Result<Value> {
    use base64::Engine;
    use std::os::windows::process::CommandExt;
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let script = match kind {
        "network" => {
            r#"$ErrorActionPreference='Stop';$adapters=@(Get-NetIPConfiguration | Select-Object -First 8 InterfaceAlias,@{n='addresses';e={@($_.IPv4Address.IPAddress)+@($_.IPv6Address.IPAddress)}},@{n='gateways';e={@(@($_.IPv4DefaultGateway.NextHop)+@($_.IPv6DefaultGateway.NextHop)|Where-Object {$_})}},@{n='dns';e={@($_.DNSServer.ServerAddresses|Where-Object {$_})}});$proxy=Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue;@{mode='local';interfaces=$adapters;interface_count=$adapters.Count;has_default_route=(@($adapters|Where-Object {$_.gateways.Count -gt 0}).Count -gt 0);has_dns_configuration=(@($adapters|Where-Object {$_.dns.Count -gt 0}).Count -gt 0);proxy_config_present=([bool]$proxy.ProxyEnable -or [bool]$proxy.AutoConfigURL);coverage='current-user OS configuration; proxy autodetection/VPN path not inferred'}|ConvertTo-Json -Depth 6 -Compress"#
        }
        "store" => {
            r#"$ErrorActionPreference='Stop';$families=@{};Get-AppxPackage|ForEach-Object {$families[$_.PackageFamilyName]=$true};@(Get-StartApps | Where-Object {$_.AppID -match '^[^!\\]+![^!\\]+$' -and $families.ContainsKey(($_.AppID -split '!')[0])} | Select-Object -First 512 @{n='name';e={$_.Name}},@{n='application_id';e={$_.AppID}},@{n='package_family';e={($_.AppID -split '!')[0]}},@{n='source';e={'packaged_current_user'}},@{n='launch_status';e={'launch_unavailable'}})|ConvertTo-Json -Depth 4 -Compress"#
        }
        "startup_user" | "startup_machine" => {
            let user = kind == "startup_user";
            if user {
                r#"$ErrorActionPreference='Stop';$items=@();foreach($source in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Run','HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce')) {if(Test-Path $source){$p=Get-ItemProperty -LiteralPath $source;foreach($v in $p.PSObject.Properties){if($v.Name -notlike 'PS*'){$items+=@{name=$v.Name;command=[string]$v.Value;source=$source;enabled='unknown'}}}}};$folder=[Environment]::GetFolderPath('Startup');if(Test-Path -LiteralPath $folder){Get-ChildItem -LiteralPath $folder -File | Select-Object -First 256 | ForEach-Object {$items+=@{name=$_.Name;path=$_.FullName;source='user_startup_folder';enabled='unknown'}}};@($items|Select-Object -First 512)|ConvertTo-Json -Depth 4 -Compress"#
            } else {
                r#"$ErrorActionPreference='Stop';$items=@();foreach($source in @('HKLM:\Software\Microsoft\Windows\CurrentVersion\Run','HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce')) {if(Test-Path $source){$p=Get-ItemProperty -LiteralPath $source;foreach($v in $p.PSObject.Properties){if($v.Name -notlike 'PS*'){$items+=@{name=$v.Name;command=[string]$v.Value;source=$source;enabled='unknown'}}}}};$folder=[Environment]::GetFolderPath('CommonStartup');if(Test-Path -LiteralPath $folder){Get-ChildItem -LiteralPath $folder -File | Select-Object -First 256 | ForEach-Object {$items+=@{name=$_.Name;path=$_.FullName;source='machine_startup_folder';enabled='unknown'}}};@($items|Select-Object -First 512)|ConvertTo-Json -Depth 4 -Compress"#
            }
        }
        _ => anyhow::bail!("unsupported"),
    };
    let script = format!("[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new();{script}");
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut system_dir = [0u16; 32768];
    let len = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            system_dir.as_mut_ptr(),
            system_dir.len() as u32,
        )
    } as usize;
    anyhow::ensure!(len > 0 && len < system_dir.len(), "provider_unavailable");
    let exe = std::path::PathBuf::from(String::from_utf16_lossy(&system_dir[..len]))
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let mut cmd = Command::new(exe);
    cmd.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &encoded,
    ])
    .creation_flags(0x08000000)
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .stdout(Stdio::piped());
    // Fixed read-only provider subprocesses have no reason to receive model credentials.
    for (key, _) in std::env::vars_os() {
        let text = key.to_string_lossy().to_uppercase();
        if text.starts_with("DAO_")
            || text.starts_with("WEBVIEW2_")
            || text.contains("API_KEY")
            || text.contains("SECRET")
            || text.ends_with("TOKEN")
        {
            cmd.env_remove(key);
        }
    }
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let began = Instant::now();
    loop {
        if cancel.is_cancelled() || began.elapsed() > Duration::from_secs(12) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            anyhow::bail!("provider_cancelled_or_timeout");
        }
        if let Some(status) = child.try_wait()? {
            let bytes = reader
                .join()
                .map_err(|_| anyhow::anyhow!("provider_failed"))??;
            anyhow::ensure!(
                status.success() && bytes.len() <= 2 * 1024 * 1024,
                "provider_failed"
            );
            if bytes.is_empty() {
                return Ok(json!([]));
            }
            return serde_json::from_slice(&bytes)
                .map_err(|_| anyhow::anyhow!("provider_invalid_response"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[cfg(not(windows))]
fn fixed_query(_: &str, _: &crate::core::Cancellation) -> anyhow::Result<Value> {
    anyhow::bail!("unsupported_platform")
}
pub fn network_config(cancel: &crate::core::Cancellation) -> anyhow::Result<Value> {
    fixed_query("network", cancel)
}
pub fn store_apps(cancel: &crate::core::Cancellation) -> Value {
    match fixed_query("store", cancel) {
        Ok(v) => {
            json!({"items":array(v),"coverage":"current-user registered Store entry identities only; no package-content scan","launch_status":"launch_unavailable"})
        }
        Err(_) => json!({"items":[],"coverage":"store_provider_unavailable","partial":true}),
    }
}
fn array(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        Value::Object(_) => vec![v],
        _ => vec![],
    }
}
pub fn startup(args: Value, cancel: &crate::core::Cancellation) -> anyhow::Result<Value> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        source: String,
        #[serde(default)]
        filter: String,
        #[serde(default = "limit")]
        limit: usize,
        #[serde(default)]
        cursor: Option<usize>,
    }
    fn limit() -> usize {
        20
    }
    let r: Request =
        serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
    anyhow::ensure!(
        matches!(r.source.as_str(), "user" | "machine")
            && (1..=512).contains(&r.limit)
            && r.filter.len() <= 128
            && r.cursor.unwrap_or(0) <= 512,
        "invalid_arguments"
    );
    let (mut rows, partial) = match fixed_query(
        if r.source == "user" {
            "startup_user"
        } else {
            "startup_machine"
        },
        cancel,
    ) {
        Ok(v) => (array(v), false),
        Err(_) => (vec![], true),
    };
    rows.retain(|v| {
        v["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains(&r.filter.to_lowercase())
    });
    rows.sort_by_key(|r| r["name"].as_str().unwrap_or("").to_owned());
    let count = rows.len();
    let offset = r.cursor.unwrap_or(0);
    let items: Vec<_> = rows.into_iter().skip(offset).take(r.limit).collect();
    let next = (offset + items.len() < count).then_some(offset + items.len());
    Ok(
        json!({"schema_version":2,"observation_id":uuid::Uuid::new_v4().to_string(),"kind":"startup_list","observed_at":crate::core::now(),"status":if partial{"partial"}else{"complete"},"data":{"items":items,"count":count,"page":{"next_cursor":next}},"coverage":{"state":if partial{"partial"}else{"provider_scoped"},"reasons":["Run/RunOnce and Startup folders only; registration is not proof of enabled state; values never executed"]},"limits":"Command strings and shortcut paths remain local, never model/workspace text. Services and scheduled tasks are outside scope."}),
    )
}
