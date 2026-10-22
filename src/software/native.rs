use super::bindings::Microsoft::Management::Deployment::*;
use serde_json::{Value, json};
use windows::Win32::System::Com::*;
use windows_core::GUID;
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
fn manager() -> windows_core::Result<PackageManager> {
    unsafe {
        CoCreateInstance(
            &GUID::from_u128(0xc53a4f16_787e_42a4_b304_29effb4bf597),
            None,
            CLSCTX_ALL,
        )
    }
}
pub fn probe() -> Value {
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() } {
        return json!({"available":false,"activation_hresult":e.code().0});
    }
    let _apartment = Apartment;
    let manager = match manager() {
        Ok(v) => v,
        Err(e) => return json!({"available":false,"activation_hresult":e.code().0}),
    };
    let sources=manager.GetPackageCatalogs().map(|rows|rows.into_iter().map(|r|match r.Info(){Ok(i)=>json!({"id":i.Id().ok().map(|s|s.to_string()),"name":i.Name().ok().map(|s|s.to_string()),"official_community_argument":i.Argument().ok().map(|s|matches!(s.to_string().as_str(),"https://cdn.winget.microsoft.com/cache"|"https://winget.azureedge.net/cache")),"type":i.Type().ok().map(|s|s.to_string())}),Err(e)=>json!({"error":e.code().0})}).collect::<Vec<_>>());
    json!({"available":true,"version":manager.Version().ok().map(|s|s.to_string()),"sources":sources.ok(),"write_validation":"NOT_VERIFIED","dns_flush":"NOT_EXECUTED"})
}
