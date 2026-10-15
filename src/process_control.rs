//! Narrow native process identity and termination; no command or model tool surface.
use crate::core::Cancellation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub created: String,
    pub image: String,
    pub owner: String,
    pub session: u32,
}
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub use native::{capture, creation, enumerate};
#[cfg(not(windows))]
pub fn creation(_: u32) -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn capture(_: u32) -> Result<Identity, &'static str> {
    Err("unsupported_platform")
}
#[cfg(not(windows))]
pub fn enumerate() -> Vec<Identity> {
    Vec::new()
}

pub fn terminate(expected: &Identity, cancel: &Cancellation) -> &'static str {
    if cancel.is_cancelled() {
        return "cancelled";
    }
    #[cfg(windows)]
    {
        native::terminate(expected, cancel)
    }
    #[cfg(not(windows))]
    {
        let _ = expected;
        "unsupported_platform"
    }
}
