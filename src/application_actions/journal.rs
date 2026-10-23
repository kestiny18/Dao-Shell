//! Dedicated minimal action receipts. An exclusive lock prevents recovery of a live dispatcher.
use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};
pub struct Journal {
    db: Connection,
    _lock: File,
}
impl Journal {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        crate::files::normalize_existing(directory)?;
        for name in ["actions.lock", "actions.db", "actions.db-journal"] {
            let p = directory.join(name);
            if let Ok(m) = std::fs::symlink_metadata(p) {
                ensure!(
                    !crate::platform::is_link(&m) && m.is_file(),
                    "journal_unavailable"
                );
            }
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("actions.lock"))?;
        lock.try_lock()?;
        let db = Connection::open(directory.join("actions.db"))?;
        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS app_actions(operation_id TEXT PRIMARY KEY, version INTEGER NOT NULL, created_at TEXT NOT NULL, body TEXT NOT NULL);")?;
        let mut j = Self { db, _lock: lock };
        for mut r in j.list()? {
            if matches!(r["lifecycle"].as_str(), Some("dispatching" | "dispatched")) {
                r["lifecycle"] = json!("unknown");
                r["recovery_reason"] = json!("recovered_without_replay");
                j.save(&r)?;
            }
        }
        j.prune(false)?;
        Ok(j)
    }
    pub fn save(&mut self, value: &Value) -> Result<()> {
        let encoded = serde_json::to_string(value)?;
        ensure!(encoded.len() < 32768, "journal_budget");
        ensure!(
            self.list()?.len() < 1024
                || self
                    .list()?
                    .iter()
                    .any(|r| r["operation_id"] == value["operation_id"]),
            "journal_budget"
        );
        self.db.execute("INSERT INTO app_actions(operation_id,version,created_at,body) VALUES (?1,1,?2,?3) ON CONFLICT(operation_id) DO UPDATE SET body=excluded.body",params![value["operation_id"].as_str().ok_or_else(||anyhow::anyhow!("invalid_receipt"))?,value["created_at"].as_str().ok_or_else(||anyhow::anyhow!("invalid_receipt"))?,encoded])?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Value>> {
        let mut s = self
            .db
            .prepare("SELECT version,body FROM app_actions ORDER BY created_at DESC LIMIT 1025")?;
        let rows = s.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (v, text) = r?;
            ensure!(v == 1 && text.len() < 32768, "invalid_receipt");
            out.push(serde_json::from_str(&text)?);
        }
        ensure!(out.len() <= 1024, "journal_budget");
        Ok(out)
    }
    pub fn prune(&mut self, all_completed: bool) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
        let mut n = 0;
        for r in self.list()? {
            // The file is retained until an explicit restore; time must not erase its proof.
            if (r["operation"] == "startup_disable" && r["lifecycle"] == "verified")
                || r["lifecycle"] == "startup_checked_retained"
            {
                continue;
            }
            if matches!(
                r["lifecycle"].as_str(),
                Some(
                    "unknown" | "dispatching" | "dispatched" | "awaiting_user" | "awaiting_reboot"
                )
            ) {
                continue;
            }
            let old = r["created_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|t| t < cutoff);
            if all_completed || old {
                n += self.db.execute(
                    "DELETE FROM app_actions WHERE operation_id=?1",
                    [r["operation_id"].as_str().unwrap_or("")],
                )?;
            }
        }
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_proof_does_not_expire_before_startup_restore() {
        let directory = tempfile::tempdir().unwrap();
        let mut journal = Journal::open(directory.path()).unwrap();
        let old = "2020-01-01T00:00:00Z";
        journal.save(&json!({"operation_id":"retained","created_at":old,"operation":"startup_disable","lifecycle":"verified","evidence":{"retained":"fixture.lnk"}})).unwrap();
        journal.save(&json!({"operation_id":"ordinary","created_at":old,"operation":"fixture","lifecycle":"verified"})).unwrap();
        assert_eq!(journal.prune(true).unwrap(), 1);
        assert_eq!(journal.list().unwrap()[0]["operation_id"], "retained");
    }
    #[test]
    fn interrupted_dispatch_recovers_unknown_never_replays_and_completed_cleanup_keeps_unknown() {
        let d = tempfile::tempdir().unwrap();
        let mut j = Journal::open(d.path()).unwrap();
        j.save(&json!({"operation_id":"one","created_at":crate::core::now(),"lifecycle":"dispatching","name":"fixture","evidence":{"code":"dispatch_outcome_unobserved","process_created":true}})).unwrap();
        assert!(Journal::open(d.path()).is_err());
        drop(j);
        let mut j = Journal::open(d.path()).unwrap();
        assert_eq!(j.list().unwrap()[0]["lifecycle"], "unknown");
        assert_eq!(j.list().unwrap()[0]["evidence"]["process_created"], true);
        assert_eq!(
            j.list().unwrap()[0]["evidence"]["code"],
            "dispatch_outcome_unobserved"
        );
        assert_eq!(j.prune(true).unwrap(), 0);
    }
}
