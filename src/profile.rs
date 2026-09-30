//! A deliberately small, local-only hardware summary with no machine identifiers.
use crate::core::{Cancellation, now};
use anyhow::Result;
use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Serialize)]
pub struct Profile {
    pub observed_at: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub os: Option<String>,
    pub cpus: Vec<String>,
    pub memory_bytes: Option<u64>,
    pub graphics: Vec<String>,
    // Volume capacities, never summed or described as physical disks.
    pub volumes: Vec<Option<u64>>,
}

fn display_text(value: &str) -> Option<String> {
    let value = value.trim_matches(|c: char| c.is_control() || c.is_whitespace());
    if value.is_empty()
        || [
            "to be filled by o.e.m.",
            "default string",
            "system product name",
            "system manufacturer",
            "unknown",
        ]
        .contains(&value.to_ascii_lowercase().as_str())
    {
        return None;
    }
    Some(
        value
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect(),
    )
}

pub fn collect(cancel: &Cancellation) -> Result<Profile> {
    cancel.check()?;
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_all();
    let mut cpus = Vec::new();
    for cpu in system.cpus() {
        if let Some(name) = display_text(cpu.brand())
            && !cpus.contains(&name)
        {
            cpus.push(name);
        }
    }
    let (brand, model, graphics) = platform();
    cancel.check()?;
    let volumes = Disks::new_with_refreshed_list()
        .iter()
        .map(|disk| {
            let size = disk.total_space();
            (size > 0).then_some(size)
        })
        .collect();
    cancel.check()?;
    Ok(Profile {
        observed_at: now(),
        brand,
        model,
        os: System::long_os_version().and_then(|s| display_text(&s)),
        cpus,
        memory_bytes: (system.total_memory() > 0).then_some(system.total_memory()),
        graphics,
        volumes,
    })
}

#[cfg(not(windows))]
fn platform() -> (Option<String>, Option<String>, Vec<String>) {
    (None, None, Vec::new())
}

#[cfg(windows)]
fn platform() -> (Option<String>, Option<String>, Vec<String>) {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        Graphics::Gdi::{DISPLAY_DEVICEW, EnumDisplayDevicesW},
        System::Registry::*,
    };
    fn bios(name: &str) -> Option<String> {
        let key: Vec<u16> = "HARDWARE\\DESCRIPTION\\System\\BIOS"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut buffer = [0u16; 512];
        let mut size = std::mem::size_of_val(&buffer) as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if result != ERROR_SUCCESS {
            return None;
        }
        display_text(&String::from_utf16_lossy(
            &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
        ))
    }
    let mut graphics = Vec::new();
    // Enumerate adapters, not monitor device IDs or registry keys.
    for index in 0..64 {
        let mut device = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) } == 0 {
            break;
        }
        let end = device
            .DeviceString
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(device.DeviceString.len());
        if let Some(name) = display_text(&String::from_utf16_lossy(&device.DeviceString[..end])) {
            graphics.push(name);
        }
    }
    (
        bios("SystemManufacturer"),
        bios("SystemProductName"),
        graphics,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknowns_and_whitelist_are_explicit() {
        assert_eq!(display_text(" To Be Filled By O.E.M. "), None);
        assert_eq!(display_text(" \n "), None);
        let p = Profile {
            observed_at: "fixture".into(),
            brand: None,
            model: None,
            os: None,
            cpus: vec![],
            memory_bytes: None,
            graphics: vec!["GPU A".into(), "GPU A".into()],
            volumes: vec![Some(1024), None],
        };
        let value = serde_json::to_value(p).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 8);
        assert!(value["memory_bytes"].is_null());
        assert_eq!(value["graphics"].as_array().unwrap().len(), 2);
        assert_eq!(value["volumes"][0], 1024);
    }
}
