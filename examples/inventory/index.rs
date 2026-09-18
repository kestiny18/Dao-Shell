use anyhow::{Context, Result, ensure};
use dao_shell::{
    core::{Cancellation, FileIdentity, now},
    files::normalize_existing,
    platform,
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, params, params_from_iter,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

const APPLICATION_ID: i64 = 0x44414f49;

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub root: PathBuf,
    pub started_at: String,
    pub finished_at: String,
    pub scan_ms: u128,
    pub entries: usize,
    pub skipped_links: usize,
    pub errors: usize,
    pub error_samples: Vec<String>,
    pub coverage: String,
    pub freshness: String,
    #[serde(default)]
    pub last_incremental_update_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Hit {
    pub path: PathBuf,
    pub identity_at_scan: FileIdentity,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub snapshot: Snapshot,
    /// SQL query and row decoding only; excludes opening the database and printing JSON.
    pub query_ms: f64,
    pub more: bool,
    pub hits: Vec<Hit>,
}

fn database_path(db: &Path) -> Result<PathBuf> {
    let absolute = if db.is_absolute() {
        db.to_owned()
    } else {
        std::env::current_dir()?.join(db)
    };
    if absolute.try_exists()? {
        let path = normalize_existing(&absolute)?;
        ensure!(path.is_file(), "数据库路径不是文件");
        return Ok(path);
    }
    let parent = normalize_existing(absolute.parent().context("数据库需要已有的父目录")?)?;
    ensure!(parent.is_dir(), "数据库父路径不是目录");
    let name = absolute.file_name().context("数据库需要文件名")?;
    dao_shell::files::validate_components(&absolute)?;
    Ok(parent.join(name))
}

fn validate_database(conn: &Connection) -> Result<()> {
    let id: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    ensure!(
        id == APPLICATION_ID && version == 1,
        "不是兼容的 Dao 索引实验数据库"
    );
    Ok(())
}

fn reader(db: &Path) -> Result<Connection> {
    let db = database_path(db)?;
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_secs(2))?;
    validate_database(&conn)?;
    Ok(conn)
}

fn read_snapshot(conn: &Connection) -> Result<Snapshot> {
    let json: String = conn
        .query_row("SELECT json FROM snapshot WHERE id=1", [], |r| r.get(0))
        .context("没有已完成的快照；请先 scan")?;
    Ok(serde_json::from_str(&json)?)
}

pub fn status(db: &Path) -> Result<Snapshot> {
    read_snapshot(&reader(db)?)
}

/// A full reconciliation baseline, not a filesystem snapshot or an event consumer.
/// A failed/cancelled/budget-limited scan never replaces the last committed generation.
pub fn scan(
    root: &Path,
    db: &Path,
    max_entries: usize,
    budget: Duration,
    cancel: &Cancellation,
) -> Result<Snapshot> {
    ensure!(max_entries > 0 && !budget.is_zero(), "扫描预算必须大于零");
    cancel.check()?;
    let root = normalize_existing(root)?;
    ensure!(root.is_dir(), "root 必须是目录");
    let db = database_path(db)?;
    ensure!(
        !db.starts_with(&root),
        "数据库必须放在扫描目录外，避免索引自身"
    );
    let existed = db.try_exists()?;
    let mut conn = Connection::open(&db)?;
    conn.busy_timeout(Duration::from_secs(2))?;
    if existed {
        validate_database(&conn)?;
    } else {
        conn.execute_batch(&format!(
            "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=1;
             CREATE TABLE entries(path TEXT PRIMARY KEY, search_path TEXT NOT NULL, identity_json TEXT NOT NULL);
             CREATE TABLE snapshot(id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);"
        ))?;
    }
    let start = Instant::now();
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous: Option<String> = tx
        .query_row("SELECT json FROM snapshot WHERE id=1", [], |r| r.get(0))
        .optional()?;
    if let Some(previous) = previous {
        let previous: Snapshot = serde_json::from_str(&previous)?;
        ensure!(
            previous.root == root,
            "一个实验数据库只绑定一个目录，请为新目录使用另一个数据库"
        );
    }
    tx.execute("DELETE FROM entries", [])?;
    let mut snapshot = Snapshot {
        root: root.clone(),
        started_at: now(),
        finished_at: String::new(),
        scan_ms: 0,
        entries: 0,
        skipped_links: 0,
        errors: 0,
        error_samples: Vec::new(),
        coverage: "selected_root_excluding_links".into(),
        freshness: "manual_snapshot_not_live".into(),
        last_incremental_update_at: None,
    };
    let mut walker = WalkDir::new(&root).follow_links(false).into_iter();
    let mut visited = 0;
    {
        let mut insert = tx.prepare("INSERT INTO entries VALUES (?1, ?2, ?3)")?;
        while let Some(entry) = walker.next() {
            cancel.check()?;
            ensure!(start.elapsed() < budget, "扫描超时；原快照保留");
            visited += 1;
            ensure!(visited <= max_entries, "超过扫描条目预算；原快照保留");
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    record_error(&mut snapshot, err.to_string());
                    continue;
                }
            };
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(err) => {
                    record_error(&mut snapshot, err.to_string());
                    if entry.file_type().is_dir() {
                        walker.skip_current_dir();
                    }
                    continue;
                }
            };
            if platform::is_link(&metadata) {
                snapshot.skipped_links += 1;
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            if entry.depth() == 0 {
                continue;
            }
            if !metadata.is_file() && !metadata.is_dir() {
                record_error(
                    &mut snapshot,
                    format!("特殊文件未收录：{}", entry.path().display()),
                );
                continue;
            }
            let identity = match platform::identity(entry.path()) {
                Ok(identity) => identity,
                Err(err) => {
                    record_error(&mut snapshot, err.to_string());
                    continue;
                }
            };
            let Some(path) = entry.path().to_str() else {
                record_error(&mut snapshot, "非 Unicode 路径未收录".into());
                continue;
            };
            let relative = entry
                .path()
                .strip_prefix(&root)?
                .to_str()
                .context("非 Unicode 相对路径")?;
            insert.execute(params![
                path,
                relative.to_lowercase(),
                serde_json::to_string(&identity)?
            ])?;
            snapshot.entries += 1;
        }
    }
    cancel.check()?;
    ensure!(start.elapsed() < budget, "扫描超时；原快照保留");
    if snapshot.errors > 0 {
        snapshot.coverage = "partial_scan_with_errors".into();
    }
    snapshot.finished_at = now();
    snapshot.scan_ms = start.elapsed().as_millis();
    tx.execute(
        "INSERT OR REPLACE INTO snapshot VALUES (1, ?1)",
        [serde_json::to_string(&snapshot)?],
    )?;
    tx.commit()?;
    Ok(snapshot)
}

fn record_error(snapshot: &mut Snapshot, error: String) {
    snapshot.errors += 1;
    if snapshot.error_samples.len() < 5 {
        snapshot.error_samples.push(error);
    }
}

/// Update only ordinary files whose current state can be checked safely. A directory,
/// missing ancestor, ambiguous path, or partial baseline requests a full reconciliation.
/// Events are hints: never trust an event's identity, kind, or ordering as filesystem truth.
pub fn refresh_paths(
    db: &Path,
    paths: &[PathBuf],
    max_entries: usize,
    cancel: &Cancellation,
) -> Result<Option<Snapshot>> {
    cancel.check()?;
    let db = database_path(db)?;
    let mut conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    validate_database(&conn)?;
    conn.busy_timeout(Duration::from_secs(2))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut snapshot = read_snapshot(&tx)?;
    if snapshot.errors > 0 || paths.len() > 1024 {
        return Ok(None);
    }
    let paths: std::collections::BTreeSet<_> = paths.iter().collect();
    for path in paths {
        cancel.check()?;
        if !path.starts_with(&snapshot.root) || path == &snapshot.root {
            return Ok(None);
        }
        if dao_shell::files::validate_components(path).is_err() {
            return Ok(None);
        }
        let Some(parent) = path.parent() else {
            return Ok(None);
        };
        // A deleted parent, link replacement or denied access must not masquerade as deletion.
        let Ok(parent) = normalize_existing(parent) else {
            return Ok(None);
        };
        if !parent.starts_with(&snapshot.root) {
            return Ok(None);
        }
        let Some(path_text) = path.to_str() else {
            return Ok(None);
        };
        let previous: Option<String> = tx
            .query_row(
                "SELECT identity_json FROM entries WHERE path=?1",
                [path_text],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let identity: FileIdentity = serde_json::from_str(&previous)?;
            if identity.directory {
                return Ok(None);
            }
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() && !platform::is_link(&metadata) => {
                let Ok(canonical) = normalize_existing(path) else {
                    return Ok(None);
                };
                if !canonical.starts_with(&snapshot.root) {
                    return Ok(None);
                }
                let Ok(identity) = platform::identity(&canonical) else {
                    return Ok(None);
                };
                if identity.directory {
                    return Ok(None);
                }
                let Some(canonical_text) = canonical.to_str() else {
                    return Ok(None);
                };
                let Some(relative) = canonical.strip_prefix(&snapshot.root)?.to_str() else {
                    return Ok(None);
                };
                tx.execute("DELETE FROM entries WHERE path=?1", [path_text])?;
                tx.execute(
                    "INSERT OR REPLACE INTO entries VALUES (?1, ?2, ?3)",
                    params![
                        canonical_text,
                        relative.to_lowercase(),
                        serde_json::to_string(&identity)?
                    ],
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tx.execute("DELETE FROM entries WHERE path=?1", [path_text])?;
            }
            _ => return Ok(None),
        }
    }
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
    snapshot.entries = usize::try_from(count)?;
    ensure!(
        snapshot.entries <= max_entries,
        "增量更新超过条目预算；原快照保留"
    );
    cancel.check()?;
    snapshot.last_incremental_update_at = Some(now());
    snapshot.freshness = "event_assisted_snapshot_not_live".into();
    tx.execute(
        "UPDATE snapshot SET json=?1 WHERE id=1",
        [serde_json::to_string(&snapshot)?],
    )?;
    tx.commit()?;
    Ok(Some(snapshot))
}

pub fn query(db: &Path, terms: &[String], limit: usize) -> Result<QueryResult> {
    ensure!((1..=200).contains(&limit), "limit 必须为 1..200");
    ensure!(
        !terms.is_empty() && terms.len() <= 12,
        "需要 1..12 个关键词"
    );
    ensure!(
        terms.iter().all(|t| !t.trim().is_empty() && t.len() <= 256),
        "关键词不能为空或超过 256 字节"
    );
    let mut conn = reader(db)?;
    let tx = conn.transaction()?;
    // Keep the manifest and rows from the same committed generation.
    let snapshot = read_snapshot(&tx)?;
    let start = Instant::now();
    let predicates = (1..=terms.len())
        .map(|i| format!("instr(search_path, ?{i}) > 0"))
        .collect::<Vec<_>>()
        .join(" AND ");
    // Baseline intentionally scans SQLite metadata, not the filesystem. No claim of sublinear search.
    let sql = format!(
        "SELECT path, identity_json FROM entries WHERE {predicates} ORDER BY path LIMIT {}",
        limit + 1
    );
    let mut statement = tx.prepare(&sql)?;
    let rows = statement.query_map(
        params_from_iter(terms.iter().map(|t| t.to_lowercase())),
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    let mut hits = Vec::new();
    for row in rows {
        let (path, identity) = row?;
        hits.push(Hit {
            path: path.into(),
            identity_at_scan: serde_json::from_str(&identity)?,
        });
    }
    let more = hits.len() > limit;
    hits.truncate(limit);
    Ok(QueryResult {
        snapshot,
        query_ms: start.elapsed().as_secs_f64() * 1000.0,
        more,
        hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(root: &Path, db: &Path) -> Snapshot {
        scan(
            root,
            db,
            1000,
            Duration::from_secs(10),
            &Cancellation::default(),
        )
        .unwrap()
    }

    #[test]
    fn incremental_batches_recheck_identity_and_roll_back_before_directory_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        let db = temp.path().join("index.db");
        fs::create_dir(&root).unwrap();
        let file = root.join("first.txt");
        fs::write(&file, "first").unwrap();
        run(&root, &db);
        let root = normalize_existing(&root).unwrap();
        let file = root.join("first.txt");
        fs::write(&file, "changed length").unwrap();
        let cancel = Cancellation::default();
        assert!(
            refresh_paths(&db, std::slice::from_ref(&file), 100, &cancel)
                .unwrap()
                .is_some()
        );
        let hit = query(&db, &["first".into()], 10).unwrap().hits.remove(0);
        assert_eq!(hit.identity_at_scan.size, 14);
        fs::remove_file(&file).unwrap();
        refresh_paths(&db, std::slice::from_ref(&file), 100, &cancel)
            .unwrap()
            .unwrap();
        assert!(query(&db, &["first".into()], 10).unwrap().hits.is_empty());
        let added = root.join("aaa.txt");
        fs::write(&added, "new").unwrap();
        let directory = root.join("zzz");
        fs::create_dir(&directory).unwrap();
        assert!(
            refresh_paths(&db, &[added.clone(), directory], 100, &cancel)
                .unwrap()
                .is_none()
        );
        assert!(
            query(&db, &["aaa".into()], 10).unwrap().hits.is_empty(),
            "partial delta must roll back before fallback"
        );
        assert!(
            refresh_paths(&db, &[temp.path().join("outside")], 100, &cancel)
                .unwrap()
                .is_none()
        );
        assert!(refresh_paths(&db, &[added], 0, &cancel).is_err());
        assert!(query(&db, &["aaa".into()], 10).unwrap().hits.is_empty());
        run(&root, &db);
        assert_eq!(query(&db, &["aaa".into()], 10).unwrap().hits.len(), 1);
    }

    #[test]
    fn restart_and_reconcile_directory_rename_delete_and_create() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        let db = temp.path().join("index.db");
        fs::create_dir_all(root.join("四川项目")).unwrap();
        fs::write(root.join("四川项目/合同.pdf"), "old").unwrap();
        run(&root, &db);
        assert_eq!(
            query(&db, &["四川".into(), "合同".into()], 20)
                .unwrap()
                .hits
                .len(),
            1
        );
        fs::rename(root.join("四川项目"), root.join("归档")).unwrap();
        // Query is explicitly a stale snapshot, including when files have disappeared.
        assert_eq!(query(&db, &["四川".into()], 20).unwrap().hits.len(), 2);
        run(&root, &db);
        assert!(query(&db, &["四川".into()], 20).unwrap().hits.is_empty());
        assert_eq!(
            query(&db, &["归档".into(), "合同".into()], 20)
                .unwrap()
                .hits
                .len(),
            1
        );
        fs::remove_file(root.join("归档/合同.pdf")).unwrap();
        fs::write(root.join("100%_final.txt"), "new").unwrap();
        run(&root, &db);
        assert!(query(&db, &["合同".into()], 20).unwrap().hits.is_empty());
        assert_eq!(query(&db, &["%_FINAL".into()], 20).unwrap().hits.len(), 1);
    }

    #[test]
    fn interrupted_rebuild_preserves_generation_and_rejects_other_databases() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        let db = temp.path().join("index.db");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("old"), "a").unwrap();
        let previous = run(&root, &db);
        fs::write(root.join("new"), "b").unwrap();
        assert!(
            scan(
                &root,
                &db,
                1,
                Duration::from_secs(10),
                &Cancellation::default()
            )
            .is_err()
        );
        assert_eq!(status(&db).unwrap().finished_at, previous.finished_at);
        assert!(query(&db, &["new".into()], 1).unwrap().hits.is_empty());
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(scan(&root, &db, 100, Duration::from_secs(10), &cancelled).is_err());
        assert!(
            scan(
                &root,
                &root.join("bad.db"),
                100,
                Duration::from_secs(10),
                &Cancellation::default()
            )
            .is_err()
        );
        assert!(!root.join("bad.db").exists());
        let other = temp.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE precious(value);")
            .unwrap();
        assert!(
            scan(
                &root,
                &other,
                100,
                Duration::from_secs(10),
                &Cancellation::default()
            )
            .is_err()
        );
        Connection::open(&other)
            .unwrap()
            .execute("INSERT INTO precious VALUES (1)", [])
            .unwrap();
    }
}
