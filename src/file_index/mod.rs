//! Platform-neutral, bounded indexing core. Not connected to desktop or a service yet.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

mod store;
pub use store::IndexStore;
#[cfg(windows)]
mod ntfs_io;
#[cfg(windows)]
pub mod windows;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeIdentity {
    pub backend: String,
    pub stable_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint(pub String);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub parent: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub directory: bool,
    pub names: Vec<Name>,
}
#[derive(Debug)]
pub enum Resolved {
    Present(Node),
    Deleted,
    Unavailable(&'static str),
}
pub(crate) fn metadata_retryable(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.to_string().as_str(),
            "retry_required: incomplete link set"
                | "retry_required: link count changed during reconciliation"
                | "retry_required: final path identity changed"
                | "retry_required: name path disappeared during reconciliation"
        )
    })
}
pub struct Enumeration {
    pub ids: Vec<String>,
    pub next: Option<String>,
}
pub struct Changes {
    pub ids: Vec<String>,
    pub next: Checkpoint,
    pub caught_up: bool,
}
/// Checkpoints and file IDs are opaque above the platform boundary.
pub trait VolumeSource {
    fn identity(&self) -> VolumeIdentity;
    fn root(&self) -> String;
    fn checkpoint(&mut self) -> Result<Checkpoint>;
    fn validate(&mut self, checkpoint: &Checkpoint) -> Result<()>;
    fn enumerate(&mut self, cursor: Option<&str>) -> Result<Enumeration>;
    fn changes(&mut self, checkpoint: &Checkpoint) -> Result<Changes>;
    fn resolve(&mut self, id: &str) -> Result<Resolved>;
}
#[derive(Clone, Default)]
pub struct Cancellation {
    flag: std::sync::Arc<AtomicBool>,
    deadline: Option<std::time::Instant>,
}
impl Cancellation {
    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        Self {
            flag: Default::default(),
            deadline: Some(std::time::Instant::now() + timeout),
        }
    }
    pub fn cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
            || self
                .deadline
                .is_some_and(|d| std::time::Instant::now() >= d)
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!self.cancelled(), "cancelled");
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct Budget {
    pub max_resolutions: usize,
    pub max_pages: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            max_resolutions: 1_000_000,
            max_pages: 10_000,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub published_unix_ms: Option<u128>,
    pub volume: VolumeIdentity,
    pub generation: u64,
    pub complete: bool,
    pub nodes: usize,
    pub names: usize,
    pub unresolved: usize,
    pub coverage_reasons: Vec<String>,
    pub checkpoint: Checkpoint,
}
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub file_id: String,
    pub components: Vec<String>,
    #[serde(skip)]
    pub(crate) row_id: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchCursor {
    pub generation: u64,
    pub after: i64,
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub items: Vec<Candidate>,
    pub next: Option<SearchCursor>,
    pub status: Status,
}

#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod usn;

#[cfg(windows)]
pub mod worker;
