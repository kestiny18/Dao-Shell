//! Local protocol. It deliberately has no database path, SQL, shell, or file mutation requests.
use super::super::SearchCursor;
use serde::{Deserialize, Serialize};
use std::time::Instant;
pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 64 * 1024;
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub id: String,
    pub timeout_ms: u64,
    pub operation: Operation,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Status,
    Rebuild {},
    Search {
        query: String,
        page_size: usize,
        cursor: Option<String>,
    },
    Cancel {
        request_id: String,
    },
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Hit {
    pub file_id: String,
    pub path: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PublicStatus {
    #[serde(default)]
    pub coverage_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<FailureDiagnostic>,
    pub phase: String,
    pub generation: Option<u64>,
    pub complete: bool,
    pub updated_unix_ms: Option<u128>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FailureDiagnostic {
    pub stage: String,
    pub code: String,
    pub native_code: Option<i32>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    pub version: u32,
    pub id: String,
    pub status: PublicStatus,
    pub hits: Vec<Hit>,
    pub cursor: Option<String>,
    pub error: Option<String>,
}
pub(super) struct Cursor {
    pub owner: super::security::Principal,
    pub query: String,
    pub cursor: SearchCursor,
    pub expires: Instant,
}
