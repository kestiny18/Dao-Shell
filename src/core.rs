use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.is_cancelled(), "本轮已取消；已经完成的改动不会撤回");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileIdentity {
    pub volume: u64,
    pub index: u64,
    pub size: u64,
    pub modified: u64,
    pub directory: bool,
}
impl FileIdentity {
    pub fn same_object(&self, other: &Self) -> bool {
        self.volume == other.volume
            && self.index == other.index
            && self.directory == other.directory
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObject {
    pub id: String,
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub observed_at: String,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Status {
    AwaitingConfirmation,
    Executing,
    Verifying,
    Succeeded,
    Failed,
    Partial,
    Unknown,
    Cancelled,
    Invalidated,
    NotStarted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub identity: FileIdentity,
    pub status: Status,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEffect {
    pub path: PathBuf,
    pub status: Status,
    pub identity: Option<FileIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub created_at: String,
    pub status: Status,
    pub confirmed_at: Option<String>,
    pub destination_anchor: PathBuf,
    pub destination_identity: FileIdentity,
    pub items: Vec<MoveItem>,
    pub directories: Vec<DirectoryEffect>,
}

impl Operation {
    pub fn aggregate(&self, cancelled: bool) -> Status {
        let statuses: Vec<_> = self
            .items
            .iter()
            .map(|x| x.status)
            .chain(self.directories.iter().map(|x| x.status))
            .collect();
        if statuses
            .iter()
            .any(|x| matches!(x, Status::Unknown | Status::Executing | Status::Verifying))
        {
            Status::Unknown
        } else if statuses.iter().all(|s| *s == Status::Succeeded) {
            Status::Succeeded
        } else if statuses.contains(&Status::Succeeded) {
            Status::Partial
        } else if cancelled {
            Status::Cancelled
        } else {
            Status::Failed
        }
    }
}

pub fn safe_multiline(input: &str) -> String {
    input
        .split('\n')
        .map(safe_text)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Escape controls and bidi overrides; keep hostile filenames from rewriting a prompt.
pub fn safe_text(input: &str) -> String {
    input.chars().flat_map(|c| {
        if c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            c.escape_unicode().collect::<Vec<_>>()
        } else { vec![c] }
    }).collect()
}
