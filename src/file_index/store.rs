use super::*;
use anyhow::Context;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
};
const APP: i32 = 0x44414931;

pub struct IndexStore {
    conn: Connection,
    observed: Option<(VolumeIdentity, String, u64, Checkpoint)>,
}
impl IndexStore {
    pub fn open_reader(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.busy_timeout(std::time::Duration::from_millis(50))?;
        let app: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(app == APP && version == 2, "unsupported index database");
        Ok(Self {
            conn,
            observed: None,
        })
    }
    pub fn enable_concurrent_reads(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=256; PRAGMA user_version=2;",
        )?;
        Ok(())
    }
    /// A new database is required. P9/P10 formats are deliberately rejected.
    pub fn open(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "database must be absolute");
        for p in path.ancestors() {
            match std::fs::symlink_metadata(p) {
                Ok(m) => {
                    ensure!(!m.file_type().is_symlink(), "linked database path");
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        ensure!(m.file_attributes() & 0x400 == 0, "reparse database path");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && p == path => (),
                Err(e) => return Err(e.into()),
            }
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let app: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        if app == 0 {
            let n: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
            ensure!(n == 0, "foreign database; no automatic migration");
            conn.execute_batch(&format!("PRAGMA application_id={APP}; PRAGMA user_version=2;
                CREATE TABLE nodes(id TEXT PRIMARY KEY,directory INTEGER NOT NULL);
                CREATE TABLE names(row_id INTEGER PRIMARY KEY, file_id TEXT NOT NULL,parent TEXT NOT NULL,name TEXT NOT NULL,folded TEXT NOT NULL,UNIQUE(file_id,parent,name));
                CREATE INDEX names_file ON names(file_id);
                CREATE TABLE unavailable(id TEXT PRIMARY KEY,reason TEXT NOT NULL);
                CREATE TABLE state(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL,root TEXT NOT NULL);"))?;
        } else {
            ensure!(app == APP, "foreign database; no automatic migration");
        }
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(version == 2, "unsupported index schema");
        Ok(Self {
            conn,
            observed: None,
        })
    }
    pub fn status(&self) -> Result<Status> {
        load(&self.conn).map(|v| v.0)
    }
    pub fn update(
        &mut self,
        source: &mut (impl VolumeSource + ?Sized),
        rebuild: bool,
        budget: Budget,
        cancel: &Cancellation,
    ) -> Result<Status> {
        cancel.check()?;
        let previous = if rebuild {
            None
        } else {
            Some(load(&self.conn)?)
        };
        if let Some((s, root)) = &previous {
            ensure!(
                s.volume == source.identity() && *root == source.root(),
                "rebuild_required: volume changed"
            );
        }
        let generation = load(&self.conn).ok().map_or(1, |v| v.0.generation + 1);
        let mut checkpoint = match &previous {
            Some((s, root)) => self
                .observed
                .as_ref()
                .filter(|(volume, saved_root, generation, _)| {
                    volume == &s.volume && saved_root == root && *generation == s.generation
                })
                .map_or_else(
                    || s.checkpoint.clone(),
                    |(_, _, _, checkpoint)| checkpoint.clone(),
                ),
            None => source.checkpoint()?,
        };
        source.validate(&checkpoint)?;
        let root = source.root();
        let tx = self.conn.transaction()?;
        let mut used = 0;
        if rebuild {
            tx.execute_batch("DELETE FROM names; DELETE FROM nodes; DELETE FROM unavailable;")?;
            let mut cursor = None;
            let mut ended = false;
            for _ in 0..budget.max_pages {
                cancel.check()?;
                let page = source.enumerate(cursor.as_deref())?;
                reconcile(&tx, source, page.ids, budget, &mut used, cancel)?;
                if page.next.is_none() {
                    ended = true;
                    break;
                }
                ensure!(cursor != page.next, "enumeration did not advance");
                cursor = page.next;
            }
            ensure!(
                ended,
                "budget_exhausted: enumeration; generation not published"
            );
        }
        let mut meaningful_changes = rebuild;
        let mut caught = false;
        for _ in 0..budget.max_pages {
            cancel.check()?;
            source.validate(&checkpoint)?;
            let changes = source.changes(&checkpoint)?;
            meaningful_changes |= reconcile(&tx, source, changes.ids, budget, &mut used, cancel)?;
            ensure!(
                changes.caught_up || changes.next != checkpoint,
                "changes did not advance"
            );
            checkpoint = changes.next;
            if changes.caught_up {
                caught = true;
                break;
            }
        }
        ensure!(
            caught,
            "budget_exhausted: catch-up; generation not published"
        );
        if !meaningful_changes {
            source.validate(&checkpoint)?;
            cancel.check()?;
            let previous = previous.expect("existing generation").0;
            self.observed = Some((
                previous.volume.clone(),
                root,
                previous.generation,
                checkpoint,
            ));
            return Ok(previous);
        }
        // Retry unresolved objects on meaningful changes; idle loops must not write their own journal.
        let unavailable = tx
            .prepare("SELECT id FROM unavailable")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        reconcile(&tx, source, unavailable, budget, &mut used, cancel)?;
        hydrate_parents(&tx, source, budget, &mut used, cancel)?;
        source.validate(&checkpoint)?;
        cancel.check()?;
        let nodes =
            tx.query_row("SELECT count(*) FROM nodes", [], |r| r.get::<_, i64>(0))? as usize;
        let names =
            tx.query_row("SELECT count(*) FROM names", [], |r| r.get::<_, i64>(0))? as usize;
        let unavailable = tx.query_row("SELECT count(*) FROM unavailable", [], |r| {
            r.get::<_, i64>(0)
        })? as usize;
        let mut unresolved = unavailable;
        let directories = directory_graph(&tx)?;
        let mut cache = HashMap::new();
        let mut stmt = tx.prepare("SELECT file_id,parent,name FROM names")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            cancel.check()?;
            let (_, p, n) = row?;
            let _ = n;
            if !parent_resolvable(&root, &p, &directories, &mut cache) {
                unresolved += 1;
            }
        }
        drop(stmt);
        let mut coverage_reasons = tx
            .prepare("SELECT DISTINCT reason FROM unavailable ORDER BY reason")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        if unresolved > unavailable {
            coverage_reasons.push("unresolved_parent_chain".into());
        }
        let status = Status {
            published_unix_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
            ),
            volume: source.identity(),
            generation,
            complete: unresolved == 0,
            nodes,
            names,
            unresolved,
            coverage_reasons,
            checkpoint,
        };
        tx.execute(
            "INSERT OR REPLACE INTO state VALUES(1,?1,?2)",
            params![serde_json::to_string(&status)?, root],
        )?;
        tx.commit()?;
        self.observed = Some((
            status.volume.clone(),
            root,
            status.generation,
            status.checkpoint.clone(),
        ));
        Ok(status)
    }
    /// Bounded results and bounded candidate examination. Cursor is invalid after an update.
    /// This core does not authorize a caller; P11.2 must filter before exposing results via IPC.
    pub fn search(
        &self,
        query: &str,
        page_size: usize,
        cursor: Option<SearchCursor>,
        cancel: &Cancellation,
    ) -> Result<Page> {
        ensure!(
            !query.is_empty() && query.len() <= 1024 && (1..=100).contains(&page_size),
            "invalid query bounds"
        );
        cancel.check()?;
        let tx = self.conn.unchecked_transaction()?;
        let (status, root) = load(&tx)?;
        if let Some(c) = &cursor {
            ensure!(c.generation == status.generation, "stale_cursor");
        }
        let control = cancel.clone();
        self.conn
            .progress_handler(1000, Some(move || control.cancelled()))?;
        let _progress = ProgressGuard(&self.conn);
        let after = cursor.map_or(0, |v| v.after);
        let mut stmt = tx.prepare("SELECT row_id,file_id,parent,name FROM names WHERE row_id>?1 AND instr(folded,?2)>0 ORDER BY row_id LIMIT 1001")?;
        let mut rows = stmt.query(params![after, query.to_lowercase()])?;
        let mut items = Vec::new();
        let mut last = after;
        let mut more = false;
        let mut inspected = 0;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            if items.len() == page_size || inspected == 1000 {
                more = true;
                break;
            }
            last = row.get(0)?;
            inspected += 1;
            let id = row.get(1)?;
            let parent: String = row.get(2)?;
            let name: String = row.get(3)?;
            if let Some(components) = resolve_path(&tx, &root, &parent, &name)? {
                items.push(Candidate {
                    file_id: id,
                    components,
                    row_id: last,
                });
            }
        }
        Ok(Page {
            items,
            next: more.then_some(SearchCursor {
                generation: status.generation,
                after: last,
            }),
            status,
        })
    }
}
fn load(conn: &Connection) -> Result<(Status, String)> {
    let (json, root): (String, String) = conn
        .query_row("SELECT json,root FROM state WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .context("no published generation")?;
    Ok((serde_json::from_str(&json)?, root))
}
fn reconcile(
    conn: &Connection,
    source: &mut (impl VolumeSource + ?Sized),
    ids: Vec<String>,
    budget: Budget,
    used: &mut usize,
    cancel: &Cancellation,
) -> Result<bool> {
    let mut changed = false;
    for id in ids.into_iter().collect::<HashSet<_>>() {
        cancel.check()?;
        let mut attempt = 0;
        let result = loop {
            cancel.check()?;
            ensure!(
                *used < budget.max_resolutions,
                "budget_exhausted: metadata; generation not published"
            );
            *used += 1;
            attempt += 1;
            match source.resolve(&id) {
                Err(error) if metadata_retryable(&error) && attempt < 3 => continue,
                result => break result?,
            }
        };
        if let Resolved::Present(node) = &result {
            ensure!(node.id == id, "source identity mismatch");
            ensure!(
                node.names.len() <= 1024 && (!node.names.is_empty() || id == source.root()),
                "invalid name set"
            );
            ensure!(
                !node.directory || node.names.len() <= 1,
                "ambiguous directory names"
            );
        }
        let existing: Option<bool> = conn
            .query_row("SELECT directory FROM nodes WHERE id=?1", [&id], |r| {
                r.get(0)
            })
            .optional()?;
        let denied: Option<String> = conn
            .query_row("SELECT reason FROM unavailable WHERE id=?1", [&id], |r| {
                r.get(0)
            })
            .optional()?;
        let unchanged = match &result {
            Resolved::Deleted => existing.is_none() && denied.is_none(),
            Resolved::Unavailable(reason) => {
                existing.is_none() && denied.as_deref() == Some(*reason)
            }
            Resolved::Present(node) if existing == Some(node.directory) && denied.is_none() => {
                let old = conn
                    .prepare("SELECT parent,name FROM names WHERE file_id=?1 ORDER BY parent,name")?
                    .query_map([&id], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                let mut names = node
                    .names
                    .iter()
                    .map(|n| (n.parent.clone(), n.name.clone()))
                    .collect::<Vec<_>>();
                names.sort();
                names.dedup();
                old == names
            }
            _ => false,
        };
        if unchanged {
            continue;
        }
        changed = true;
        conn.execute("DELETE FROM names WHERE file_id=?1", [&id])?;
        conn.execute("DELETE FROM nodes WHERE id=?1", [&id])?;
        conn.execute("DELETE FROM unavailable WHERE id=?1", [&id])?;
        match result {
            Resolved::Deleted => (),
            Resolved::Unavailable(reason) => {
                conn.execute("INSERT INTO unavailable VALUES(?1,?2)", params![id, reason])?;
            }
            Resolved::Present(node) => {
                conn.execute(
                    "INSERT INTO nodes VALUES(?1,?2)",
                    params![id, node.directory],
                )?;
                for name in node.names {
                    ensure!(
                        !name.name.is_empty()
                            && !name.name.contains(['\\', '/', '\0'])
                            && name.name != "."
                            && name.name != "..",
                        "invalid name component"
                    );
                    conn.execute("INSERT OR IGNORE INTO names(file_id,parent,name,folded) VALUES(?1,?2,?3,?4)",params![id,name.parent,name.name,name.name.to_lowercase()])?;
                }
            }
        }
    }
    Ok(changed)
}
fn hydrate_parents(
    conn: &Connection,
    source: &mut (impl VolumeSource + ?Sized),
    budget: Budget,
    used: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    let mut seen = HashSet::new();
    loop {
        let ids = conn
            .prepare(
                "SELECT DISTINCT parent FROM names WHERE parent NOT IN (SELECT id FROM nodes)",
            )?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| id != &source.root() && seen.insert(id.clone()))
            .collect();
        if ids.is_empty() {
            break;
        }
        reconcile(conn, source, ids, budget, used, cancel)?;
    }
    Ok(())
}
fn resolve_path(
    conn: &Connection,
    root: &str,
    parent: &str,
    name: &str,
) -> Result<Option<Vec<String>>> {
    let mut parts = VecDeque::from([name.to_owned()]);
    let mut current = parent.to_owned();
    let mut seen = HashSet::new();
    for _ in 0..256 {
        if current == root {
            return Ok(Some(parts.into()));
        }
        if !seen.insert(current.clone()) {
            return Ok(None);
        }
        let row:Option<(String,String)> = conn.query_row("SELECT n.parent,n.name FROM names n JOIN nodes f ON f.id=n.file_id WHERE f.id=?1 AND f.directory=1",[&current],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        match row {
            Some((p, n)) => {
                parts.push_front(n);
                current = p;
            }
            None => return Ok(None),
        }
    }
    Ok(None)
}

struct ProgressGuard<'a>(&'a Connection);
impl Drop for ProgressGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}
fn directory_graph(conn: &Connection) -> Result<HashMap<String, String>> {
    Ok(conn.prepare("SELECT n.file_id,n.parent FROM names n JOIN nodes f ON f.id=n.file_id WHERE f.directory=1")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<HashMap<_,_>>>()?)
}
fn parent_resolvable(
    root: &str,
    parent: &str,
    dirs: &HashMap<String, String>,
    cache: &mut HashMap<String, Option<usize>>,
) -> bool {
    let mut current = parent;
    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    let result = loop {
        if current == root {
            break Some(0);
        }
        if let Some(value) = cache.get(current) {
            break *value;
        }
        if !seen.insert(current) || chain.len() >= 256 {
            break None;
        }
        chain.push(current);
        match dirs.get(current) {
            Some(next) => current = next,
            None => break None,
        }
    };
    let mut depth = result;
    for id in chain.into_iter().rev() {
        depth = depth.and_then(|d| if d < 255 { Some(d + 1) } else { None });
        cache.insert(id.to_owned(), depth);
    }
    depth.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn noop_keeps_publication_time_and_wal_readers_see_committed_generation() {
        let (dir, mut store, mut source) = fixture();
        source.put("a", false, &[("root", "alpha")]);
        source.enumeration.push("a".into());
        store.enable_concurrent_reads().unwrap();
        let first = update(&mut store, &mut source, true);
        source.changed.push("a".into());
        let unchanged = update(&mut store, &mut source, false);
        assert_eq!(first.generation, unchanged.generation);
        assert_eq!(first.published_unix_ms, unchanged.published_unix_ms);
        assert!(first.published_unix_ms.is_some());
        let path = store.conn.path().unwrap().to_owned();
        let reader = IndexStore::open_reader(Path::new(&path)).unwrap();
        let tx = store.conn.transaction().unwrap();
        tx.execute("UPDATE names SET name='uncommitted'", [])
            .unwrap();
        assert_eq!(
            reader
                .search("alpha", 1, None, &Cancellation::default())
                .unwrap()
                .items[0]
                .components
                .last()
                .unwrap(),
            "alpha"
        );
        drop(tx);
        drop(store);
        let reopened = IndexStore::open(Path::new(&path)).unwrap();
        assert_eq!(
            reopened.status().unwrap().published_unix_ms,
            first.published_unix_ms
        );
        drop(dir);
    }
    #[test]
    fn sqlite_no_match_scan_observes_deadline_inside_vm() {
        let (_dir, mut store, mut source) = fixture();
        update(&mut store, &mut source, true);
        // Deliberately expensive no-match scan: no rows reach the Rust iteration loop.
        store.conn.execute_batch("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000) INSERT INTO names(file_id,parent,name,folded) SELECT CAST(x AS TEXT),'root',CAST(x AS TEXT),hex(zeroblob(256)) FROM n;").unwrap();
        let started = std::time::Instant::now();
        let result = store.search(
            "never-matches",
            1,
            None,
            &Cancellation::with_timeout(std::time::Duration::from_millis(1)),
        );
        let error = result.unwrap_err();
        assert!(format!("{error:#}").contains("interrupt"), "{error:#}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        let explicit = Cancellation::default();
        let signal = explicit.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            signal.cancel();
        });
        assert!(
            format!(
                "{:#}",
                store
                    .search("never-matches", 1, None, &explicit)
                    .unwrap_err()
            )
            .contains("interrupt")
        );
        trigger.join().unwrap();
        assert!(store.search("0", 1, None, &Cancellation::default()).is_ok());
    }
    struct Fake {
        cancel_on_resolve: Option<Cancellation>,
        transient_failures: usize,
        transient_message: &'static str,
        seen_checkpoints: Vec<String>,
        nodes: HashMap<String, Node>,
        enumeration: Vec<String>,
        changed: Vec<String>,
        cursor: u32,
        gap: bool,
        bad: bool,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                cancel_on_resolve: None,
                transient_failures: 0,
                transient_message: "retry_required: incomplete link set",
                seen_checkpoints: Vec::new(),
                nodes: HashMap::new(),
                enumeration: Vec::new(),
                changed: Vec::new(),
                cursor: 1,
                gap: false,
                bad: false,
            }
        }
        fn put(&mut self, id: &str, directory: bool, names: &[(&str, &str)]) {
            self.nodes.insert(
                id.into(),
                Node {
                    id: id.into(),
                    directory,
                    names: names
                        .iter()
                        .map(|(p, n)| Name {
                            parent: (*p).into(),
                            name: (*n).into(),
                        })
                        .collect(),
                },
            );
        }
    }
    impl VolumeSource for Fake {
        fn identity(&self) -> VolumeIdentity {
            VolumeIdentity {
                backend: "fake".into(),
                stable_id: "fixture".into(),
            }
        }
        fn root(&self) -> String {
            "root".into()
        }
        fn checkpoint(&mut self) -> Result<Checkpoint> {
            Ok(Checkpoint(self.cursor.to_string()))
        }
        fn validate(&mut self, _: &Checkpoint) -> Result<()> {
            ensure!(!self.gap, "rebuild_required: gap");
            Ok(())
        }
        fn enumerate(&mut self, _: Option<&str>) -> Result<Enumeration> {
            Ok(Enumeration {
                ids: self.enumeration.clone(),
                next: None,
            })
        }
        fn changes(&mut self, checkpoint: &Checkpoint) -> Result<Changes> {
            self.seen_checkpoints.push(checkpoint.0.clone());
            ensure!(!self.bad, "unsupported USN version");
            Ok(Changes {
                ids: self.changed.clone(),
                next: Checkpoint(self.cursor.to_string()),
                caught_up: true,
            })
        }
        fn resolve(&mut self, id: &str) -> Result<Resolved> {
            if let Some(cancel) = self.cancel_on_resolve.take() {
                cancel.cancel();
            }
            if self.transient_failures > 0 {
                self.transient_failures -= 1;
                anyhow::bail!("{}", self.transient_message);
            }
            if id == "denied" {
                return Ok(Resolved::Unavailable("access_denied"));
            }
            Ok(self
                .nodes
                .get(id)
                .cloned()
                .map(Resolved::Present)
                .unwrap_or(Resolved::Deleted))
        }
    }
    fn fixture() -> (tempfile::TempDir, IndexStore, Fake) {
        let d = tempfile::tempdir().unwrap();
        let s = IndexStore::open(&d.path().join("index.db")).unwrap();
        (d, s, Fake::new())
    }
    #[test]
    fn identity_races_retry_with_bounds_and_never_skip_unreconciled_events() {
        for message in [
            "retry_required: incomplete link set",
            "retry_required: name path disappeared during reconciliation",
        ] {
            let (_dir, mut store, mut source) = fixture();
            source.transient_message = message;
            source.put("a", false, &[("root", "original")]);
            source.enumeration.push("a".into());
            update(&mut store, &mut source, true);
            source.put("a", false, &[("root", "renamed")]);
            source.changed.push("a".into());
            source.cursor = 2;
            source.transient_failures = 3;
            assert!(
                store
                    .update(
                        &mut source,
                        false,
                        Budget::default(),
                        &Cancellation::default()
                    )
                    .is_err()
            );
            assert_eq!(source.transient_failures, 0);
            assert_eq!(store.status().unwrap().checkpoint.0, "1");
            assert_eq!(store.observed.as_ref().unwrap().3.0, "1");
            assert_eq!(paths(&store, "original").len(), 1);
            source.transient_failures = 2;
            update(&mut store, &mut source, false);
            assert_eq!(paths(&store, "renamed").len(), 1);
            assert_eq!(store.status().unwrap().checkpoint.0, "2");
            source.transient_failures = 2;
            let error = store
                .update(
                    &mut source,
                    false,
                    Budget {
                        max_resolutions: 1,
                        max_pages: 10,
                    },
                    &Cancellation::default(),
                )
                .unwrap_err();
            assert!(error.to_string().contains("budget_exhausted"));
            assert!(!metadata_retryable(&anyhow::anyhow!(
                "unsupported USN version"
            )));
            assert!(!metadata_retryable(&anyhow::anyhow!(
                "rebuild_required: journal gap"
            )));
        }
    }
    #[test]
    fn observed_checkpoint_advances_without_writes_but_failures_and_restart_replay() {
        let (dir, mut store, mut source) = fixture();
        update(&mut store, &mut source, true);
        source.cursor = 2;
        update(&mut store, &mut source, false);
        assert_eq!(store.status().unwrap().checkpoint.0, "1");
        source.cursor = 3;
        source.bad = true;
        assert!(
            store
                .update(
                    &mut source,
                    false,
                    Budget::default(),
                    &Cancellation::default()
                )
                .is_err()
        );
        assert_eq!(source.seen_checkpoints.last().unwrap(), "2");
        source.bad = false;
        source.put("a", false, &[("root", "changed")]);
        source.changed.push("a".into());
        update(&mut store, &mut source, false);
        assert_eq!(source.seen_checkpoints.last().unwrap(), "2");
        assert_eq!(store.status().unwrap().checkpoint.0, "3");
        source.changed.clear();
        source.cursor = 4;
        update(&mut store, &mut source, false);
        drop(store);
        let mut reopened = IndexStore::open(&dir.path().join("index.db")).unwrap();
        update(&mut reopened, &mut source, false);
        assert_eq!(source.seen_checkpoints.last().unwrap(), "3");
        assert_eq!(reopened.status().unwrap().checkpoint.0, "3");
    }
    fn update(s: &mut IndexStore, f: &mut Fake, rebuild: bool) -> Status {
        s.update(f, rebuild, Budget::default(), &Cancellation::default())
            .unwrap()
    }
    fn paths(s: &IndexStore, q: &str) -> Vec<Vec<String>> {
        s.search(q, 100, None, &Cancellation::default())
            .unwrap()
            .items
            .into_iter()
            .map(|v| v.components)
            .collect()
    }
    #[test]
    fn missing_parent_hydration_and_unicode_directory_rename() {
        let (_d, mut s, mut f) = fixture();
        f.put("dir", true, &[("root", "中文目录")]);
        f.put("file", false, &[("dir", "资料.txt")]);
        f.enumeration = vec!["file".into()];
        assert!(update(&mut s, &mut f, true).complete);
        assert_eq!(paths(&s, "资料"), vec![vec!["中文目录", "资料.txt"]]);
        f.put("dir", true, &[("root", "新目录")]);
        f.changed = vec!["dir".into()];
        f.cursor = 2;
        update(&mut s, &mut f, false);
        assert_eq!(paths(&s, "资料"), vec![vec!["新目录", "资料.txt"]]);
    }
    #[test]
    fn hardlink_names_rename_remove_and_final_delete() {
        let (_d, mut s, mut f) = fixture();
        f.put("file", false, &[("root", "one.txt"), ("root", "two.txt")]);
        f.enumeration = vec!["file".into()];
        assert_eq!(update(&mut s, &mut f, true).names, 2);
        assert_eq!(paths(&s, ".txt").len(), 2);
        f.put("file", false, &[("root", "renamed.txt")]);
        f.changed = vec!["file".into()];
        update(&mut s, &mut f, false);
        assert!(paths(&s, "one").is_empty());
        assert_eq!(paths(&s, "renamed").len(), 1);
        f.nodes.remove("file");
        update(&mut s, &mut f, false);
        assert!(paths(&s, ".txt").is_empty());
    }
    #[test]
    fn unknown_parent_and_cycles_never_claim_complete() {
        let (_d, mut s, mut f) = fixture();
        f.put("file", false, &[("denied", "secret.txt")]);
        f.put("loop", true, &[("loop", "cycle")]);
        f.enumeration = vec!["file".into(), "loop".into()];
        let state = update(&mut s, &mut f, true);
        assert!(!state.complete);
        assert!(state.unresolved >= 2);
        assert!(paths(&s, "secret").is_empty());
    }
    #[test]
    fn deleted_parent_is_not_guessed_as_root() {
        let (_d, mut s, mut f) = fixture();
        f.put("dir", true, &[("root", "dir")]);
        f.put("file", false, &[("dir", "file")]);
        f.enumeration = vec!["dir".into(), "file".into()];
        update(&mut s, &mut f, true);
        f.nodes.remove("dir");
        f.changed = vec!["dir".into()];
        assert!(!update(&mut s, &mut f, false).complete);
        assert!(paths(&s, "file").is_empty());
    }
    #[test]
    fn gaps_and_unknown_versions_preserve_published_generation() {
        let (_d, mut s, mut f) = fixture();
        f.put("file", false, &[("root", "before")]);
        f.enumeration = vec!["file".into()];
        let state = update(&mut s, &mut f, true);
        f.put("file", false, &[("root", "after")]);
        f.gap = true;
        assert!(
            s.update(&mut f, false, Budget::default(), &Cancellation::default())
                .is_err()
        );
        f.gap = false;
        f.bad = true;
        assert!(
            s.update(&mut f, true, Budget::default(), &Cancellation::default())
                .is_err()
        );
        assert_eq!(s.status().unwrap().generation, state.generation);
        assert_eq!(paths(&s, "before").len(), 1);
    }
    #[test]
    fn budgets_and_cancel_do_not_publish_partial_generation() {
        let (_d, mut s, mut f) = fixture();
        f.put("file", false, &[("root", "file")]);
        f.enumeration = vec!["file".into()];
        assert!(
            s.update(
                &mut f,
                true,
                Budget {
                    max_resolutions: 0,
                    max_pages: 1
                },
                &Cancellation::default()
            )
            .is_err()
        );
        assert!(s.status().is_err());
        let c = Cancellation::default();
        c.cancel();
        assert!(s.update(&mut f, true, Budget::default(), &c).is_err());
        assert!(s.status().is_err());
    }
    #[test]
    fn pagination_is_bounded_and_stale_cursor_rejected() {
        let (_d, mut s, mut f) = fixture();
        for i in 0..5 {
            let id = i.to_string();
            f.put(&id, false, &[("root", &format!("file{i}"))]);
            f.enumeration.push(id);
        }
        update(&mut s, &mut f, true);
        let first = s.search("file", 2, None, &Cancellation::default()).unwrap();
        assert_eq!(first.items.len(), 2);
        assert!(first.next.is_some());
        let second = s
            .search("file", 2, first.next.clone(), &Cancellation::default())
            .unwrap();
        assert_eq!(second.items.len(), 2);
        assert!(
            first
                .items
                .iter()
                .all(|a| second.items.iter().all(|b| a.file_id != b.file_id))
        );
        f.put("0", false, &[("root", "file0-renamed")]);
        f.changed.push("0".into());
        update(&mut s, &mut f, false);
        assert!(
            s.search("file", 2, first.next, &Cancellation::default())
                .is_err()
        );
        assert!(
            s.search("file", 101, None, &Cancellation::default())
                .is_err()
        );
    }
    #[test]
    fn reopened_database_replays_without_enumeration() {
        let (d, mut s, mut f) = fixture();
        f.put("file", false, &[("root", "old")]);
        f.enumeration = vec!["file".into()];
        update(&mut s, &mut f, true);
        drop(s);
        f.enumeration.clear();
        f.put("file", false, &[("root", "new")]);
        f.changed = vec!["file".into()];
        f.cursor = 8;
        let mut s = IndexStore::open(&d.path().join("index.db")).unwrap();
        assert_eq!(update(&mut s, &mut f, false).checkpoint.0, "8");
        assert_eq!(paths(&s, "new").len(), 1);
    }
    #[test]
    fn database_failure_rolls_back_names_and_checkpoint() {
        let (_d, mut s, mut f) = fixture();
        f.put("file", false, &[("root", "old")]);
        f.enumeration = vec!["file".into()];
        update(&mut s, &mut f, true);
        s.conn.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON names WHEN NEW.name='fail' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        f.put("file", false, &[("root", "fail")]);
        f.changed = vec!["file".into()];
        f.cursor = 2;
        assert!(
            s.update(&mut f, false, Budget::default(), &Cancellation::default())
                .is_err()
        );
        assert_eq!(s.status().unwrap().checkpoint.0, "1");
        assert_eq!(paths(&s, "old").len(), 1);
        // An explicit rebuild deletes/repopulates inside the same transaction:
        // publication failure and cancellation still retain the previous generation.
        assert!(
            s.update(&mut f, true, Budget::default(), &Cancellation::default())
                .is_err()
        );
        assert_eq!(s.status().unwrap().generation, 1);
        assert_eq!(paths(&s, "old").len(), 1);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(s.update(&mut f, true, Budget::default(), &cancel).is_err());
        assert_eq!(s.status().unwrap().checkpoint.0, "1");
        s.conn.execute_batch("DROP TRIGGER fail").unwrap();
        let during = Cancellation::default();
        f.cancel_on_resolve = Some(during.clone());
        assert!(s.update(&mut f, true, Budget::default(), &during).is_err());
        assert!(during.cancelled());
        assert_eq!(s.status().unwrap().generation, 1);
        assert_eq!(paths(&s, "old").len(), 1);
    }
    #[test]
    fn old_schema_rejected_without_changes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("old.db");
        let c = Connection::open(&p).unwrap();
        c.execute_batch("PRAGMA application_id=1145130830; CREATE TABLE records(name TEXT); INSERT INTO records VALUES('keep');").unwrap();
        drop(c);
        assert!(IndexStore::open(&p).is_err());
        let c = Connection::open(p).unwrap();
        assert_eq!(
            c.query_row("SELECT name FROM records", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "keep"
        );
    }
}
