#[cfg(windows)]
mod bindings;
pub mod cli_export;
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub use native::probe;
#[cfg(not(windows))]
pub fn probe() -> serde_json::Value {
    serde_json::json!({"available":false,"code":"unsupported_platform"})
}
pub mod contract;
pub mod dns;
pub mod engine;
pub mod startup;
pub mod store;
#[cfg(windows)]
pub mod typed;
