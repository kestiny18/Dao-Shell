use crate::core::{Operation, Status};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

pub struct Journal {
    db: Connection,
    _lock: File,
}
impl Journal {
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory).context("无法创建本地数据目录")?;
        crate::files::normalize_existing(directory)?;
        for name in ["operations.lock", "operations.db", "operations.db-journal"] {
            let path = directory.join(name);
            if let Ok(m) = fs::symlink_metadata(&path) {
                ensure!(
                    !crate::platform::is_link(&m) && m.is_file(),
                    "操作记录路径不是普通文件"
                );
            }
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("operations.lock"))?;
        lock.try_lock()
            .context("此数据目录已被另一个 Dao-Shell 占用")?;
        let db = Connection::open(directory.join("operations.db"))?;
        db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY, created_at TEXT NOT NULL, body TEXT NOT NULL);")?;
        let mut result = Self { db, _lock: lock };
        result.prune()?;
        Ok(result)
    }
    pub fn save(&mut self, operation: &Operation) -> Result<()> {
        self.db
            .execute(
                "INSERT INTO operations(id,created_at,body) VALUES (?1,?2,?3)
            ON CONFLICT(id) DO UPDATE SET body=excluded.body",
                params![
                    operation.id,
                    operation.created_at,
                    serde_json::to_string(operation)?
                ],
            )
            .context("操作记录写入失败；已停止继续修改，可重启后核对")?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Operation>> {
        let mut statement = self
            .db
            .prepare("SELECT body FROM operations ORDER BY created_at DESC")?;
        let values = statement.query_map([], |row| row.get::<_, String>(0))?;
        values.map(|s| Ok(serde_json::from_str(&s?)?)).collect()
    }
    fn prune(&mut self) -> Result<()> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
        for operation in self.list()? {
            if chrono::DateTime::parse_from_rfc3339(&operation.created_at)? < cutoff
                && is_terminal(operation.status)
            {
                self.db
                    .execute("DELETE FROM operations WHERE id=?1", [&operation.id])?;
            }
        }
        Ok(())
    }
    pub fn clear_completed(&mut self) -> Result<usize> {
        let mut count = 0;
        for operation in self.list()? {
            if is_terminal(operation.status) {
                count += self
                    .db
                    .execute("DELETE FROM operations WHERE id=?1", [&operation.id])?;
            }
        }
        self.db.execute_batch("VACUUM;")?;
        Ok(count)
    }
}
fn is_terminal(status: Status) -> bool {
    matches!(
        status,
        Status::Succeeded
            | Status::Partial
            | Status::Failed
            | Status::Cancelled
            | Status::Invalidated
    )
}
