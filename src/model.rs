//! Provider configuration, transport and synthetic diagnostics shared by all entries.
pub mod budget;
mod diagnostics;
pub mod provider;
mod transport;
pub(crate) use diagnostics::check_client;
pub use diagnostics::{ConnectionCheck, check_connection};
pub(crate) use transport::{ModelClient, ModelUnavailable, TOOL_LIMIT};
