//! Desktop opt-in metadata snapshots. No filesystem actions or model authority.
mod watch;
use crate::{
    core::{Cancellation, FileIdentity, now},
    files::{Scope, normalize_existing},
    platform,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct BudgetExceeded;
impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "达到 10 万条目 / 60 秒预算，请缩小目录")
    }
}
impl std::error::Error for BudgetExceeded {}

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Registration {
    id: String,
    root: PathBuf,
    identity: FileIdentity,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub root: PathBuf,
    pub state: String,
    pub entries: usize,
    pub updated_at: Option<String>,
    pub message: String,
    pub full_scans: usize,
    pub incremental_batches: usize,
}
struct Entry {
    registration: Registration,
    status: Status,
    records: Option<Arc<Vec<Record>>>,
    cancel: Cancellation,
    generation: u64,
}
struct Inner {
    entries: BTreeMap<String, Entry>,
    error: Option<String>,
    closed: bool,
}
pub struct Inventory {
    directory: PathBuf,
    inner: Mutex<Inner>,
    scanner: Mutex<()>,
    #[cfg(test)]
    hooks: TestHooks,
}
#[cfg(test)]
#[derive(Default)]
struct TestHooks {
    armed: std::sync::atomic::AtomicBool,
    fail_watch: std::sync::atomic::AtomicBool,
    fail_scan: std::sync::atomic::AtomicBool,
    fail_budget: std::sync::atomic::AtomicBool,
    overflow: std::sync::atomic::AtomicBool,
    period_ms: std::sync::atomic::AtomicU64,
    retry_ms: std::sync::atomic::AtomicU64,
}
impl Inventory {
    pub fn new(directory: PathBuf) -> Arc<Self> {
        let value = Arc::new(Self {
            directory,
            inner: Mutex::new(Inner {
                entries: BTreeMap::new(),
                error: None,
                closed: false,
            }),
            scanner: Mutex::new(()),
            #[cfg(test)]
            hooks: TestHooks::default(),
        });
        if let Err(e) = value.restore() {
            let mut inner = value.inner.lock().unwrap();
            inner.entries.clear();
            inner.error = Some(format!("目录记录无法加载：{e}；原文件保留"));
        }
        value
    }
    fn restore(&self) -> Result<()> {
        let path = self.directory.join("inventory.json");
        if !path.exists() {
            return Ok(());
        }
        normalize_existing(&path)?;
        let registrations: Vec<Registration> = serde_json::from_slice(&fs::read(path)?)?;
        ensure!(registrations.len() <= 32, "最多登记 32 个目录");
        let mut inner = self.inner.lock().unwrap();
        for r in registrations {
            uuid::Uuid::parse_str(&r.id)?;
            ensure!(r.root.is_absolute(), "登记目录必须为绝对路径");
            crate::files::validate_components(&r.root)?;
            ensure!(!inner.entries.contains_key(&r.id), "重复目录记录标识");
            let mut status = Status {
                id: r.id.clone(),
                root: r.root.clone(),
                state: "pending".into(),
                full_scans: 0,
                incremental_batches: 0,
                entries: 0,
                updated_at: None,
                message: "等待启动校准；当前搜索使用现场扫描".into(),
            };
            let records = match self.load_snapshot(&r) {
                Ok(Some((records, updated))) => {
                    status.entries = records.len();
                    status.updated_at = Some(updated);
                    Some(Arc::new(records))
                }
                Ok(None) => None,
                Err(err) => {
                    status.state = "damaged".into();
                    status.message = format!("缓存损坏；请立即更新重建。{err}");
                    None
                }
            };
            inner.entries.insert(
                r.id.clone(),
                Entry {
                    registration: r,
                    status,
                    records,
                    cancel: Cancellation::default(),
                    generation: 0,
                },
            );
        }
        Ok(())
    }
    fn load_snapshot(&self, r: &Registration) -> Result<Option<(Vec<Record>, String)>> {
        let path = self.database(&r.id);
        if !path.exists() {
            return Ok(None);
        }
        normalize_existing(&path)?;
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let app: i64 = conn.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        ensure!(app == 1145130826, "缓存标识不匹配");
        let json: String = conn.query_row("SELECT json FROM snapshot", [], |row| row.get(0))?;
        let (saved, updated): (Registration, String) = serde_json::from_str(&json)?;
        ensure!(
            saved.root == r.root && saved.identity.same_object(&r.identity),
            "缓存目录不匹配"
        );
        let mut statement = conn.prepare("SELECT json FROM records LIMIT 100001")?;
        let mut records = Vec::new();
        for row in statement.query_map([], |row| row.get::<_, String>(0))? {
            let record: Record = serde_json::from_str(&row?)?;
            ensure!(record.path.starts_with(&r.root), "缓存路径超出登记目录");
            crate::files::validate_components(&record.path)?;
            records.push(record);
        }
        ensure!(records.len() <= 100_000, "缓存超出条目预算");
        Ok(Some((records, updated)))
    }
    pub fn restrict(&self, scope: &Scope) {
        let mut inner = self.inner.lock().unwrap();
        for e in inner.entries.values_mut() {
            if scope.check(&e.registration.root, false).is_err() {
                e.cancel.cancel();
                e.generation += 1;
                e.records = None;
                e.status.state = "unavailable".into();
                e.status.message = "读取权限或目录已变化".into();
            }
        }
    }
    fn save(&self, inner: &Inner) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        normalize_existing(&self.directory)?;
        let values: Vec<_> = inner.entries.values().map(|e| &e.registration).collect();
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        file.write_all(&serde_json::to_vec_pretty(&values)?)?;
        file.as_file().sync_all()?;
        file.persist(self.directory.join("inventory.json"))?;
        Ok(())
    }
    fn database(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.sqlite"))
    }
    pub fn statuses(&self, scope: &Scope) -> Result<Vec<Status>> {
        let inner = self.inner.lock().unwrap();
        ensure!(
            inner.error.is_none(),
            "{}",
            inner.error.as_deref().unwrap_or_default()
        );
        Ok(inner
            .entries
            .values()
            .map(|e| {
                let mut s = e.status.clone();
                if scope.check(&e.registration.root, false).is_err() {
                    s.root = PathBuf::from("（当前无访问权限）");
                    s.state = "unavailable".into();
                    s.message = "读取权限或目录已变化；可移除记录".into();
                }
                if s.state != "unavailable"
                    && (fs::read_dir(&e.registration.root).is_err()
                        || !platform::identity(&e.registration.root)
                            .is_ok_and(|actual| actual.same_object(&e.registration.identity)))
                {
                    s.state = "unavailable".into();
                    s.message = "目录不可访问或身份已变化；请重新登记".into();
                }
                s
            })
            .collect())
    }
    pub fn add(self: &Arc<Self>, path: &Path, scope: &Scope) -> Result<String> {
        let root = scope.check(path, false)?;
        ensure!(root.is_dir(), "请选择普通目录");
        fs::create_dir_all(&self.directory)?;
        let directory = normalize_existing(&self.directory)?;
        ensure!(
            !directory.starts_with(&root),
            "索引缓存必须位于扫描目录之外，请选择更小的目录"
        );
        let identity = platform::identity(&root)?;
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut inner = self.inner.lock().unwrap();
            ensure!(inner.error.is_none(), "目录登记文件损坏，无法修改");
            ensure!(!inner.closed, "入口正在退出");
            ensure!(inner.entries.len() < 32, "最多登记 32 个目录");
            ensure!(
                !inner
                    .entries
                    .values()
                    .any(|e| root.starts_with(&e.registration.root)
                        || e.registration.root.starts_with(&root)),
                "该目录与已有加速目录重叠，请先移除重叠项"
            );
            inner.entries.insert(
                id.clone(),
                Entry {
                    registration: Registration {
                        id: id.clone(),
                        root: root.clone(),
                        identity,
                    },
                    status: Status {
                        id: id.clone(),
                        root,
                        state: "pending".into(),
                        full_scans: 0,
                        incremental_batches: 0,
                        entries: 0,
                        updated_at: None,
                        message: String::new(),
                    },
                    records: None,
                    cancel: Cancellation::default(),
                    generation: 0,
                },
            );
            if let Err(e) = self.save(&inner) {
                inner.entries.remove(&id);
                return Err(e);
            }
        }
        self.update(&id, scope)?;
        Ok(id)
    }
    pub fn resume(self: &Arc<Self>, scope: &Scope) {
        let ids: Vec<_> = self
            .inner
            .lock()
            .unwrap()
            .entries
            .values()
            .filter(|e| e.status.state != "damaged")
            .map(|e| e.registration.id.clone())
            .collect();
        for id in ids {
            let _ = self.update(&id, scope);
        }
    }
    pub fn update(self: &Arc<Self>, id: &str, scope: &Scope) -> Result<()> {
        let (r, cancel, generation) = {
            let mut inner = self.inner.lock().unwrap();
            ensure!(!inner.closed, "入口正在退出");
            let e = inner.entries.get_mut(id).context("目录记录已移除")?;
            let root = scope.check(&e.registration.root, false)?;
            ensure!(
                e.registration
                    .identity
                    .same_object(&platform::identity(&root)?),
                "目录身份已变化，请移除后重新登记"
            );
            if e.status.state == "building" {
                return Ok(());
            }
            e.cancel.cancel();
            e.cancel = Cancellation::default();
            e.generation += 1;
            e.status.state = "building".into();
            e.status.message = "后台记录中；此范围暂用现场扫描".into();
            (e.registration.clone(), e.cancel.clone(), e.generation)
        };
        let manager = Arc::downgrade(self);
        std::thread::spawn(move || watch::run(manager, r, cancel, generation));
        Ok(())
    }
    fn working(&self, r: &Registration, generation: u64) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let Some(e) = inner.entries.get_mut(&r.id) else {
            return false;
        };
        if e.generation != generation || e.cancel.is_cancelled() {
            return false;
        }
        e.status.state = "building".into();
        e.status.message = "后台更新中；此范围暂用现场扫描".into();
        true
    }
    fn publish(
        &self,
        r: &Registration,
        generation: u64,
        result: Result<(Vec<Record>, String, tempfile::NamedTempFile)>,
        incremental: bool,
        watching: bool,
        settled: bool,
    ) -> Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        let Some(e) = inner.entries.get_mut(&r.id) else {
            return Ok(false);
        };
        if e.generation != generation || e.cancel.is_cancelled() {
            return Ok(false);
        }
        let (records, updated, temp) = result?;
        temp.persist(self.database(&r.id))?;
        e.status.entries = records.len();
        e.status.updated_at = Some(updated);
        e.status.state = if !watching {
            "pending"
        } else if settled {
            "ready"
        } else {
            "building"
        }
        .into();
        e.status.message = if !watching {
            "监听不可用；已保留快照，搜索使用现场扫描，稍后重试"
        } else if settled {
            "事件辅助快照，仍可能有延迟；每 5 分钟校准"
        } else {
            "正在补齐记录期间收到的变化；此范围暂用现场扫描"
        }
        .into();
        if incremental {
            e.status.incremental_batches += 1;
        } else {
            e.status.full_scans += 1;
        }
        e.records = Some(Arc::new(records));
        Ok(true)
    }
    fn failed(&self, r: &Registration, generation: u64, error: &anyhow::Error, paused: bool) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(e) = inner.entries.get_mut(&r.id)
            && e.generation == generation
            && !e.cancel.is_cancelled()
        {
            e.status.state = "pending".into();
            e.status.message = format!(
                "更新失败，旧记录保留；现场扫描。{}：{error}",
                if paused {
                    "自动更新已暂停，请缩小目录后手动更新"
                } else {
                    "稍后退避重试"
                }
            );
        }
    }
    fn scan(
        &self,
        r: &Registration,
        cancel: &Cancellation,
    ) -> Result<(Vec<Record>, String, tempfile::NamedTempFile)> {
        cancel.check()?;
        let root = normalize_existing(&r.root)?;
        ensure!(
            r.identity.same_object(&platform::identity(&root)?),
            "目录身份变化"
        );
        let cache = normalize_existing(&self.directory)?;
        ensure!(!cache.starts_with(&root), "缓存不得位于扫描目录内");
        let temp = tempfile::NamedTempFile::new_in(&self.directory)?;
        let mut conn = Connection::open(temp.path())?;
        conn.execute_batch("PRAGMA application_id=1145130826; CREATE TABLE records(path TEXT PRIMARY KEY, name TEXT NOT NULL, json TEXT NOT NULL); CREATE TABLE snapshot(json TEXT NOT NULL);")?;
        let tx = conn.transaction()?;
        let start = Instant::now();
        let mut records = Vec::new();
        let mut walker = walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_depth(32)
            .into_iter();
        let mut visited = 0;
        while let Some(item) = walker.next() {
            cancel.check()?;
            visited += 1;
            if visited > 100_000 || start.elapsed() >= Duration::from_secs(60) {
                return Err(BudgetExceeded.into());
            }
            let item = item?;
            let metadata = fs::symlink_metadata(item.path())?;
            if platform::is_link(&metadata) {
                if item.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            ensure!(
                item.depth() != 32 || !metadata.is_dir(),
                "目录超过 32 层，记录不完整，请缩小目录"
            );
            ensure!(
                metadata.is_file() || metadata.is_dir(),
                "存在不支持的特殊文件"
            );
            let path = item.path().to_path_buf();
            let name = item
                .file_name()
                .to_str()
                .context("存在非 Unicode 文件名")?
                .to_lowercase();
            let record = Record {
                path: path.clone(),
                identity: platform::identity(&path)?,
                modified: metadata
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<chrono::Utc>::from),
            };
            tx.execute(
                "INSERT INTO records VALUES (?1,?2,?3)",
                params![
                    path.to_str().context("路径编码不支持")?,
                    name,
                    serde_json::to_string(&record)?
                ],
            )?;
            records.push(record);
        }
        cancel.check()?;
        if start.elapsed() >= Duration::from_secs(60) {
            return Err(BudgetExceeded.into());
        }
        ensure!(
            r.identity
                .same_object(&platform::identity(&normalize_existing(&root)?)?),
            "扫描期间目录身份变化"
        );
        let updated = now();
        tx.execute(
            "INSERT INTO snapshot VALUES (?1)",
            [serde_json::to_string(&(r, &updated))?],
        )?;
        tx.commit()?;
        drop(conn);
        temp.as_file().sync_all()?;
        Ok((records, updated, temp))
    }
    pub fn stop(&self, id: &str) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let e = inner.entries.get_mut(id).context("目录已移除")?;
        e.cancel.cancel();
        e.generation += 1;
        e.status.state = "pending".into();
        e.status.message =
            "已停止自动更新；旧缓存保留，当前使用现场扫描。立即更新或下次启动恢复".into();
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let e = inner.entries.remove(id).context("目录已移除")?;
        e.cancel.cancel();
        if let Err(err) = self.save(&inner) {
            inner.entries.insert(id.into(), e);
            return Err(err);
        }
        let db = self.database(id);
        if db.exists() {
            fs::remove_file(db)?;
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        for e in inner.entries.values() {
            e.cancel.cancel();
        }
    }
    /// All requested roots must be covered; never reduce the user's search range.
    pub fn records(&self, roots: &[PathBuf], scope: &Scope) -> Option<Vec<Record>> {
        self.records_with_reason(roots, scope).ok()
    }
    pub fn records_with_reason(
        &self,
        roots: &[PathBuf],
        scope: &Scope,
    ) -> std::result::Result<Vec<Record>, &'static str> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| "现场扫描：加速目录状态暂不可用。")?;
        let mut result = Vec::new();
        for root in roots {
            scope
                .check(root, false)
                .map_err(|_| "现场扫描：读取范围或目录访问权限已变化。")?;
            fs::read_dir(root).map_err(|_| "现场扫描：登记目录当前无法读取。")?;
            let e = inner
                .entries
                .values()
                .find(|e| root == &e.registration.root)
                .ok_or("现场扫描：部分查询目录尚未登记加速，未缩小搜索范围。")?;
            if e.status.state != "ready" {
                return Err("现场扫描：目录记录正在更新、待更新或不可用。");
            }
            scope
                .check(&e.registration.root, false)
                .map_err(|_| "现场扫描：登记目录当前不在读取权限内。")?;
            let actual = platform::identity(
                &normalize_existing(&e.registration.root)
                    .map_err(|_| "现场扫描：登记目录路径已变化。")?,
            )
            .map_err(|_| "现场扫描：无法检查登记目录身份。")?;
            if !actual.same_object(&e.registration.identity) {
                return Err("现场扫描：登记目录身份已变化，请重新登记。");
            }
            result.extend(
                e.records
                    .as_ref()
                    .ok_or("现场扫描：尚无已提交快照。")?
                    .iter()
                    .filter(|r| r.path.starts_with(root))
                    .cloned(),
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{self, Objects, Search};
    fn fixture() -> (tempfile::TempDir, PathBuf, Scope, Arc<Inventory>) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        for (name, text) in [
            ("alpha.txt", "one"),
            ("ALPHA.csv", "longer"),
            ("beta.txt", "third"),
        ] {
            fs::write(root.join(name), text).unwrap();
        }
        let scope = Scope::new(std::slice::from_ref(&root), &[]).unwrap();
        let inv = Inventory::new(temp.path().join("cache"));
        (temp, root, scope, inv)
    }
    fn wait(inv: &Inventory, scope: &Scope, id: &str) {
        let start = Instant::now();
        loop {
            let rows = inv.statuses(scope).unwrap();
            let row = rows.iter().find(|r| r.id == id).unwrap();
            if row.state != "building" {
                assert_eq!(row.state, "ready", "{}", row.message);
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn query(inv: &Arc<Inventory>, scope: &Scope, value: serde_json::Value) -> files::SearchResult {
        let mut objects = Objects::default();
        objects.set_inventory(inv.clone());
        files::search(
            &serde_json::from_value(value).unwrap(),
            scope,
            &mut objects,
            &Cancellation::default(),
        )
        .unwrap()
    }
    #[test]
    fn persistent_search_parity_fallback_and_identity() {
        let (temp, root, scope, inv) = fixture();
        let id = inv.add(&root, &scope).unwrap();
        wait(&inv, &scope, &id);
        for sort in ["relevance", "name", "modified_desc", "size_desc"] {
            for page in 0..3 {
                let value = serde_json::json!({"query":"alpha","sort":sort,"page":page,"limit":1});
                let request: Search = serde_json::from_value(value.clone()).unwrap();
                let scan = files::search(
                    &request,
                    &scope,
                    &mut Objects::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                let indexed = query(&inv, &scope, value);
                assert_eq!(indexed.source, "inventory");
                assert_eq!(
                    indexed.items.iter().map(|o| &o.path).collect::<Vec<_>>(),
                    scan.items.iter().map(|o| &o.path).collect::<Vec<_>>()
                );
                assert_eq!(indexed.has_more, scan.has_more);
            }
        }
        assert_eq!(
            query(
                &inv,
                &scope,
                serde_json::json!({"terms":["alpha"],"kind":"file"})
            )
            .source,
            "scan"
        );
        let another = temp.path().join("other");
        fs::create_dir(&another).unwrap();
        let mixed = Scope::new(&[root.clone(), another.clone()], &[]).unwrap();
        assert_eq!(
            query(&inv, &mixed, serde_json::json!({"query":"alpha"})).source,
            "scan"
        );
        assert_eq!(
            query(
                &inv,
                &mixed,
                serde_json::json!({"query":"alpha","directory":root})
            )
            .source,
            "inventory"
        );
        let narrow = Scope::new(&[another], &[]).unwrap();
        inv.restrict(&narrow);
        assert!(
            inv.statuses(&narrow).unwrap()[0]
                .root
                .to_string_lossy()
                .contains("当前无访问权限")
        );
        assert!(inv.records(scope.read_roots(), &narrow).is_none());
        inv.update(&id, &scope).unwrap();
        wait(&inv, &scope, &id);
        fs::remove_file(root.join("alpha.txt")).unwrap();
        fs::write(root.join("alpha.txt"), "replacement").unwrap();
        let result = query(&inv, &scope, serde_json::json!({"query":"alpha"}));
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.skipped, 1);
        let restored = Inventory::new(temp.path().join("cache"));
        assert!(restored.statuses(&scope).unwrap()[0].entries > 0);
        assert_eq!(
            query(&restored, &scope, serde_json::json!({"query":"alpha"})).source,
            "scan"
        );
        restored.resume(&scope);
        wait(&restored, &scope, &id);
        assert_eq!(
            query(&restored, &scope, serde_json::json!({"query":"alpha"}))
                .items
                .len(),
            2
        );
        restored.shutdown();
        inv.shutdown();
    }
    #[test]
    fn cancel_remove_late_publish_and_corrupt_cache() {
        let (temp, root, scope, inv) = fixture();
        let id = inv.add(&root, &scope).unwrap();
        wait(&inv, &scope, &id);
        let original = fs::read(inv.database(&id)).unwrap();
        {
            let _block = inv.scanner.lock().unwrap();
            inv.update(&id, &scope).unwrap();
            inv.stop(&id).unwrap();
        }
        {
            let _settled = inv.scanner.lock().unwrap();
        }
        assert_eq!(fs::read(inv.database(&id)).unwrap(), original);
        {
            let _block = inv.scanner.lock().unwrap();
            inv.update(&id, &scope).unwrap();
            inv.remove(&id).unwrap();
        }
        {
            let _settled = inv.scanner.lock().unwrap();
        }
        assert!(!inv.database(&id).exists());
        assert!(inv.statuses(&scope).unwrap().is_empty());
        assert_eq!(fs::read_to_string(root.join("alpha.txt")).unwrap(), "one");
        let id = inv.add(&root, &scope).unwrap();
        wait(&inv, &scope, &id);
        inv.shutdown();
        fs::write(inv.database(&id), "corrupt fixture").unwrap();
        let restored = Inventory::new(temp.path().join("cache"));
        assert_eq!(restored.statuses(&scope).unwrap()[0].state, "damaged");
        restored.resume(&scope);
        assert_eq!(restored.statuses(&scope).unwrap()[0].state, "damaged");
        restored.update(&id, &scope).unwrap();
        wait(&restored, &scope, &id);
        restored.shutdown();
    }
    #[test]
    fn overlapping_and_inside_cache_rejected() {
        let (temp, root, scope, inv) = fixture();
        let id = inv.add(&root, &scope).unwrap();
        wait(&inv, &scope, &id);
        assert!(inv.add(&root, &scope).is_err());
        let all = Scope::new(&[temp.path().to_owned()], &[]).unwrap();
        assert!(inv.add(temp.path(), &all).is_err());
        inv.shutdown();
    }
    fn until(mut condition: impl FnMut() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(
                started.elapsed() < Duration::from_secs(8),
                "condition timed out"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    fn native_notifications_incremental_directory_reconcile_and_restart() {
        let (temp, root, scope, inv) = fixture();
        let id = inv.add(&root, &scope).unwrap();
        wait(&inv, &scope, &id);
        let started = Instant::now();
        fs::write(root.join("fresh.txt"), "new").unwrap();
        until(|| {
            query(&inv, &scope, serde_json::json!({"query":"fresh"}))
                .items
                .len()
                == 1
                && inv.statuses(&scope).unwrap()[0].incremental_batches > 0
        });
        assert!(started.elapsed() < Duration::from_secs(5));
        eprintln!(
            "native create to indexed result: {}ms",
            started.elapsed().as_millis()
        );
        fs::write(root.join("fresh.txt"), "updated contents longer").unwrap();
        until(|| {
            let q = query(&inv, &scope, serde_json::json!({"query":"fresh"}));
            q.source == "inventory" && q.items.first().is_some_and(|r| r.identity.size == 23)
        });
        fs::rename(root.join("fresh.txt"), root.join("renamed.txt")).unwrap();
        until(|| {
            let q = query(&inv, &scope, serde_json::json!({"query":"renamed"}));
            q.source == "inventory" && q.items.len() == 1
        });
        fs::remove_file(root.join("renamed.txt")).unwrap();
        until(|| {
            let q = query(&inv, &scope, serde_json::json!({"query":"renamed"}));
            q.source == "inventory" && q.matched_in_scan == 0
        });
        let before = inv.statuses(&scope).unwrap()[0].full_scans;
        let reconcile_started = Instant::now();
        fs::create_dir(root.join("folder")).unwrap();
        fs::write(root.join("folder/inside.txt"), "inside").unwrap();
        until(|| inv.statuses(&scope).unwrap()[0].full_scans > before);
        eprintln!(
            "directory change to reconciled snapshot: {}ms",
            reconcile_started.elapsed().as_millis()
        );
        fs::rename(root.join("folder"), root.join("changed-folder")).unwrap();
        until(|| {
            let q = query(&inv, &scope, serde_json::json!({"query":"inside"}));
            q.source == "inventory"
                && q.items
                    .first()
                    .is_some_and(|o| o.path.to_string_lossy().contains("changed-folder"))
        });
        inv.shutdown();
        fs::write(root.join("offline.txt"), "offline").unwrap();
        let restored = Inventory::new(temp.path().join("cache"));
        restored.resume(&scope);
        wait(&restored, &scope, &id);
        assert_eq!(
            query(&restored, &scope, serde_json::json!({"query":"offline"}))
                .items
                .len(),
            1
        );
        restored.shutdown();
    }
    #[test]
    fn bootstrap_notifications_overflow_failure_backoff_and_budget_stop() {
        use std::sync::atomic::Ordering::SeqCst;
        let (_temp, root, scope, inv) = fixture();
        inv.hooks.retry_ms.store(1000, SeqCst);
        let id;
        {
            let _gate = inv.scanner.lock().unwrap();
            id = inv.add(&root, &scope).unwrap();
            until(|| inv.hooks.armed.load(SeqCst));
            fs::write(root.join("during-bootstrap.txt"), "during").unwrap();
        }
        wait(&inv, &scope, &id);
        until(|| inv.statuses(&scope).unwrap()[0].incremental_batches > 0);
        let before = inv.statuses(&scope).unwrap()[0].full_scans;
        inv.hooks.overflow.store(true, SeqCst);
        until(|| inv.statuses(&scope).unwrap()[0].full_scans > before);
        let original = fs::read(inv.database(&id)).unwrap();
        let before = inv.statuses(&scope).unwrap()[0].full_scans;
        inv.hooks.fail_scan.store(true, SeqCst);
        inv.hooks.overflow.store(true, SeqCst);
        until(|| {
            inv.statuses(&scope).unwrap()[0]
                .message
                .contains("injected scan failure")
        });
        assert_eq!(fs::read(inv.database(&id)).unwrap(), original);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(inv.statuses(&scope).unwrap()[0].full_scans, before);
        until(|| inv.statuses(&scope).unwrap()[0].full_scans > before);
        inv.hooks.fail_budget.store(true, SeqCst);
        inv.hooks.overflow.store(true, SeqCst);
        until(|| {
            inv.statuses(&scope).unwrap()[0]
                .message
                .contains("自动更新已暂停")
        });
        let count = inv.statuses(&scope).unwrap()[0].full_scans;
        fs::write(root.join("after-budget.txt"), "after").unwrap();
        std::thread::sleep(Duration::from_millis(1300));
        assert_eq!(inv.statuses(&scope).unwrap()[0].full_scans, count);
        inv.update(&id, &scope).unwrap();
        wait(&inv, &scope, &id);
        assert_eq!(
            query(&inv, &scope, serde_json::json!({"query":"after-budget"}))
                .items
                .len(),
            1
        );
        inv.stop(&id).unwrap();
        let updated = inv.statuses(&scope).unwrap()[0].updated_at.clone();
        fs::write(root.join("stopped.txt"), "stopped").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(inv.statuses(&scope).unwrap()[0].updated_at, updated);
        assert_eq!(inv.statuses(&scope).unwrap()[0].state, "pending");
        inv.shutdown();
    }
    #[test]
    fn watch_failure_degrades_then_recovers_and_periodic_reconcile_runs() {
        use std::sync::atomic::Ordering::SeqCst;
        let (_temp, root, scope, inv) = fixture();
        inv.hooks.fail_watch.store(true, SeqCst);
        inv.hooks.retry_ms.store(600, SeqCst);
        inv.hooks.period_ms.store(800, SeqCst);
        let id = inv.add(&root, &scope).unwrap();
        until(|| {
            inv.statuses(&scope).unwrap()[0]
                .message
                .contains("监听不可用")
        });
        assert_eq!(
            query(&inv, &scope, serde_json::json!({"query":"alpha"})).source,
            "scan"
        );
        inv.hooks.fail_watch.store(false, SeqCst);
        until(|| inv.statuses(&scope).unwrap()[0].state == "ready");
        let count = inv.statuses(&scope).unwrap()[0].full_scans;
        until(|| inv.statuses(&scope).unwrap()[0].full_scans > count);
        inv.remove(&id).unwrap();
        fs::write(root.join("after-remove.txt"), "safe").unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        assert!(!inv.database(&id).exists());
        inv.shutdown();
    }
}
