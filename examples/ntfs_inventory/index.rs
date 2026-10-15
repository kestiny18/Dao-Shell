//! FRN graph experiment, one name per MFT record (NOT all hard-link paths).
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

const APP_ID: i32 = 0x44414f4e;
const DELETE: u32 = 0x200;
const RENAME_OLD: u32 = 0x1000;
const RENAME_NEW: u32 = 0x2000;
const HARD_LINK: u32 = 0x10000;

#[derive(Clone, Debug)]
struct Record {
    id: u64,
    parent: u64,
    usn: i64,
    reason: u32,
    attributes: u32,
    name: String,
}

fn u16le(b: &[u8], n: usize) -> u16 {
    u16::from_le_bytes(b[n..n + 2].try_into().unwrap())
}
fn u32le(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn u64le(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(b[n..n + 8].try_into().unwrap())
}

fn parse(bytes: &[u8]) -> Result<(u64, Vec<Record>)> {
    ensure!(bytes.len() >= 8, "truncated response header");
    let next = u64le(bytes, 0);
    let mut pos = 8;
    let mut records = Vec::new();
    while pos < bytes.len() {
        ensure!(bytes.len() - pos >= 8, "truncated USN header");
        let size = u32le(bytes, pos) as usize;
        ensure!(
            size >= 60 && size <= bytes.len() - pos && size.is_multiple_of(8),
            "invalid USN record length"
        );
        let b = &bytes[pos..pos + size];
        ensure!(
            u16le(b, 4) == 2 && u16le(b, 6) == 0,
            "unsupported USN record version {}.{}; rebuild with compatible reader required",
            u16le(b, 4),
            u16le(b, 6)
        );
        let len = u16le(b, 56) as usize;
        let offset = u16le(b, 58) as usize;
        ensure!(
            offset >= 60
                && offset.is_multiple_of(2)
                && len.is_multiple_of(2)
                && offset + len <= size,
            "invalid USN filename bounds"
        );
        let utf16: Vec<u16> = b[offset..offset + len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let name = String::from_utf16(&utf16)
            .context("unsupported non-Unicode filename; refusing silent loss")?;
        ensure!(
            !name.is_empty() && !name.contains(['\\', '/', '\0']),
            "invalid filename component"
        );
        records.push(Record {
            id: u64le(b, 8),
            parent: u64le(b, 16),
            usn: u64le(b, 24) as i64,
            reason: u32le(b, 40),
            attributes: u32le(b, 52),
            name,
        });
        pos += size;
    }
    Ok((next, records))
}

#[derive(Serialize, Deserialize, Clone)]
struct State {
    volume: String,
    volume_guid: String,
    serial: u32,
    journal_id: u64,
    next_usn: i64,
}
fn check_cursor(
    state: &State,
    serial: u32,
    id: u64,
    first: i64,
    lowest: i64,
    next: i64,
) -> Result<()> {
    ensure!(
        state.serial == serial,
        "rebuild_required: volume identity changed"
    );
    ensure!(
        state.journal_id == id,
        "rebuild_required: journal ID changed"
    );
    ensure!(
        state.next_usn >= first.max(lowest) && state.next_usn <= next,
        "rebuild_required: cursor outside valid journal range"
    );
    Ok(())
}
fn open(db: &Path, create: bool) -> Result<Connection> {
    // This experiment's metadata must stay in an explicit existing local experiment directory.
    ensure!(db.is_absolute(), "db must be an absolute path");
    let mut ancestor = Some(db);
    while let Some(p) = ancestor {
        if let Ok(m) = std::fs::symlink_metadata(p) {
            ensure!(
                !m.file_type().is_symlink(),
                "database path must not contain links"
            );
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    m.file_attributes() & 0x400 == 0,
                    "database path must not contain reparse points"
                );
            }
        }
        ancestor = p.parent();
    }
    let flags = if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(db, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let app: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if app == 0 && create {
        let tables: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
        ensure!(tables == 0, "not an empty experiment database");
        conn.execute_batch(&format!("PRAGMA application_id={APP_ID}; PRAGMA user_version=1; CREATE TABLE records(id TEXT PRIMARY KEY,parent TEXT NOT NULL,usn INTEGER NOT NULL,attributes INTEGER NOT NULL,name TEXT NOT NULL,search_name TEXT NOT NULL); CREATE TABLE state(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);"))?;
    } else {
        ensure!(app == APP_ID, "not an NTFS experiment database");
    }
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    ensure!(version == 1, "unsupported database version");
    Ok(conn)
}
fn upsert(conn: &Connection, r: &Record) -> Result<()> {
    conn.execute("INSERT INTO records VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET parent=excluded.parent,usn=excluded.usn,attributes=excluded.attributes,name=excluded.name,search_name=excluded.search_name WHERE excluded.usn>=records.usn",params![format!("{:016x}",r.id),format!("{:016x}",r.parent),r.usn,r.attributes,r.name,r.name.to_lowercase()])?;
    Ok(())
}
fn apply(conn: &Connection, r: &Record) -> Result<()> {
    ensure!(
        r.reason & HARD_LINK == 0,
        "rebuild_required: hard-link change needs name reconciliation; no cursor committed"
    );
    if r.reason & DELETE != 0 {
        conn.execute(
            "DELETE FROM records WHERE id=?1 AND usn<=?2",
            params![format!("{:016x}", r.id), r.usn],
        )?;
    } else if r.reason & RENAME_OLD == 0 || r.reason & RENAME_NEW != 0 {
        upsert(conn, r)?;
    }
    Ok(())
}
fn load(conn: &Connection) -> Result<State> {
    let s: String = conn
        .query_row("SELECT json FROM state WHERE id=1", [], |r| r.get(0))
        .context("no completed snapshot")?;
    Ok(serde_json::from_str(&s)?)
}
fn save(conn: &Connection, state: &State) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO state VALUES(1,?1)",
        [serde_json::to_string(state)?],
    )?;
    Ok(())
}

#[cfg(windows)]
fn catch_up(conn: &Connection, file: &std::fs::File, state: &mut State) -> Result<usize> {
    use super::windows;
    let j = windows::journal(file)?;
    ensure!(
        state.volume_guid == windows::guid(&state.volume)?,
        "rebuild_required: mounted volume GUID changed"
    );
    check_cursor(
        state,
        windows::identity(&state.volume)?,
        j.UsnJournalID,
        j.FirstUsn,
        j.LowestValidUsn,
        j.NextUsn,
    )?;
    let target = j.NextUsn;
    let mut count = 0;
    while state.next_usn < target {
        let bytes = windows::read(file, state.journal_id, state.next_usn)
            .context("read_journal; snapshot and cursor rolled back")?;
        let (next, records) = parse(&bytes)?;
        ensure!(
            next <= i64::MAX as u64 && next as i64 > state.next_usn,
            "journal did not advance"
        );
        for record in records {
            ensure!(
                record.usn >= state.next_usn && record.usn < (next as i64),
                "record outside returned cursor range"
            );
            apply(conn, &record)?;
            count += 1;
        }
        state.next_usn = next as i64;
    }
    let j = windows::journal(file)?;
    ensure!(
        state.volume_guid == windows::guid(&state.volume)?,
        "rebuild_required: mounted volume GUID changed"
    );
    check_cursor(
        state,
        windows::identity(&state.volume)?,
        j.UsnJournalID,
        j.FirstUsn,
        j.LowestValidUsn,
        j.NextUsn,
    )?;
    Ok(count)
}

#[cfg(windows)]
pub fn build(volume: &str, db: &Path) -> Result<Value> {
    use super::windows;
    let started = Instant::now();
    let serial = windows::identity(volume)?;
    let file = windows::open(volume, windows_sys::Win32::Foundation::GENERIC_READ)?;
    let j = windows::journal(&file)?;
    let mut state = State {
        volume: volume.to_ascii_uppercase(),
        volume_guid: windows::guid(volume)?,
        serial,
        journal_id: j.UsnJournalID,
        next_usn: j.NextUsn,
    };
    let mut conn = open(db, true)?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM records", [])?;
    let mut next = 0;
    let mut enumerated = 0u64;
    let mut pages = 0;
    loop {
        let bytes = match windows::enumeration(&file, next) {
            Ok(b) => b,
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .and_then(|v| v.raw_os_error())
                    == Some(38) =>
            {
                break;
            }
            Err(e) => return Err(e.context("enum_mft; snapshot and cursor rolled back")),
        };
        let (cursor, records) = parse(&bytes)?;
        ensure!(cursor > next, "MFT cursor did not advance");
        for r in records {
            upsert(&tx, &r)?;
            enumerated += 1;
        }
        next = cursor;
        pages += 1;
    }
    let enum_ms = started.elapsed().as_millis();
    let replayed = catch_up(&tx, &file, &mut state)?;
    save(&tx, &state)?;
    tx.commit()?;
    let mut result = status(db)?;
    result["enumerated_records"] = json!(enumerated);
    result["enum_pages"] = json!(pages);
    result["enum_ms"] = json!(enum_ms);
    result["initial_replayed_records"] = json!(replayed);
    result["ready_ms"] = json!(started.elapsed().as_millis());
    result["process_memory_bytes_after_build"] = json!(memory_bytes());
    Ok(result)
}

#[cfg(windows)]
fn memory_bytes() -> Option<u64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        false,
        sysinfo::ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(pid).map(|process| process.memory())
}
#[cfg(windows)]
pub fn replay(db: &Path) -> Result<Value> {
    use super::windows;
    let started = Instant::now();
    let mut conn = open(db, false)?;
    let tx = conn.transaction()?;
    let mut state = load(&tx)?;
    let file = windows::open(&state.volume, windows_sys::Win32::Foundation::GENERIC_READ)?;
    let old = state.next_usn;
    let events = catch_up(&tx, &file, &mut state)?;
    save(&tx, &state)?;
    tx.commit()?;
    Ok(
        json!({"status":"replayed","events":events,"old_usn":old,"next_usn":state.next_usn,"replay_ms":started.elapsed().as_millis()}),
    )
}

type Graph = HashMap<String, (String, String, u32)>;
fn graph(conn: &Connection) -> Result<Graph> {
    let mut stmt = conn.prepare("SELECT id,parent,name,attributes FROM records")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            (
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u32>(3)?,
            ),
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<Graph>>()?)
}
fn resolve(id: &str, g: &Graph) -> Option<String> {
    let mut current = id;
    let mut seen = HashSet::new();
    let mut parts = Vec::new();
    loop {
        if !seen.insert(current) {
            return None;
        }
        // NTFS root directory's MFT record number is 5 (upper 16 bits are sequence).
        if u64::from_str_radix(current, 16).ok()? & 0x0000_ffff_ffff_ffff == 5 {
            break;
        }
        let (parent, name, _) = g.get(current)?;
        parts.push(name.as_str());
        current = parent;
    }
    parts.reverse();
    Some(parts.join("\\"))
}
pub fn status(db: &Path) -> Result<Value> {
    let conn = open(db, false)?;
    let tx = conn.unchecked_transaction()?;
    let state = load(&tx)?;
    let g = graph(&tx)?;
    let unresolved = g.keys().filter(|k| resolve(k, &g).is_none()).count();
    let directories = g.values().filter(|v| v.2 & 0x10 != 0).count();
    let reparse = g.values().filter(|v| v.2 & 0x400 != 0).count();
    Ok(
        json!({"status":"snapshot","volume":state.volume,"volume_serial":state.serial,"journal_id":state.journal_id.to_string(),"next_usn":state.next_usn,"records":g.len(),"directories":directories,"reparse_records":reparse,"unresolved_paths":unresolved,"db_bytes":std::fs::metadata(db)?.len(),"coverage":"one_name_per_mft_record_not_all_hardlinks","fields":"name,parent_frn,frn,attributes,usn; no file size or mtime","freshness":"through_saved_usn_not_live"}),
    )
}
pub fn query(db: &Path, query: &str) -> Result<Value> {
    ensure!(!query.is_empty(), "query cannot be empty");
    let started = Instant::now();
    let conn = open(db, false)?;
    let tx = conn.unchecked_transaction()?;
    let state = load(&tx)?;
    let mut stmt = tx.prepare("SELECT id FROM records WHERE instr(search_name,?1)>0")?;
    let ids = stmt
        .query_map([query.to_lowercase()], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Do not emit real names or paths. Resolve matching parent chains on demand only.
    let mut resolvable = 0;
    for id in &ids {
        let mut g = Graph::new();
        let mut current = id.clone();
        loop {
            if g.contains_key(&current)
                || u64::from_str_radix(&current, 16)? & 0x0000_ffff_ffff_ffff == 5
            {
                break;
            }
            let row = tx.query_row(
                "SELECT parent,name,attributes FROM records WHERE id=?1",
                [&current],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                    ))
                },
            );
            match row {
                Ok(value) => {
                    let parent = value.0.clone();
                    g.insert(current, value);
                    current = parent;
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => break,
                Err(e) => return Err(e.into()),
            }
        }
        if resolve(id, &g).is_some() {
            resolvable += 1;
        }
    }
    Ok(
        json!({"matched":ids.len(),"resolvable":resolvable,"query_ms":started.elapsed().as_secs_f64()*1000.0,"next_usn":state.next_usn,"mode":"filename_literal_substring_counts_only"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rec(id: u64, parent: u64, name: &str, usn: i64, reason: u32) -> Record {
        Record {
            id,
            parent,
            name: name.into(),
            usn,
            reason,
            attributes: 0,
        }
    }
    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("index.db"), true).unwrap();
        (dir, conn)
    }
    #[test]
    fn malformed_records_are_rejected() {
        for bytes in [vec![], vec![0; 7], vec![0; 16]] {
            assert!(parse(&bytes).is_err());
        }
        let mut bytes = vec![0u8; 72];
        bytes[8..12].copy_from_slice(&64u32.to_le_bytes());
        bytes[12..14].copy_from_slice(&3u16.to_le_bytes());
        assert!(
            parse(&bytes)
                .unwrap_err()
                .to_string()
                .contains("unsupported")
        );
        bytes[12..14].copy_from_slice(&2u16.to_le_bytes());
        bytes[64..66].copy_from_slice(&2u16.to_le_bytes());
        bytes[66..68].copy_from_slice(&60u16.to_le_bytes());
        bytes[68..70].copy_from_slice(&(b'a' as u16).to_le_bytes());
        assert_eq!(parse(&bytes).unwrap().1[0].name, "a");
        bytes[64..66].copy_from_slice(&100u16.to_le_bytes());
        assert!(parse(&bytes).is_err());
    }
    #[test]
    fn journal_gaps_and_volume_replacement_require_rebuild() {
        let s = State {
            volume: "E:".into(),
            volume_guid: "synthetic-volume".into(),
            serial: 1,
            journal_id: 2,
            next_usn: 100,
        };
        assert!(check_cursor(&s, 1, 2, 0, 0, 101).is_ok());
        for tuple in [
            (2, 2, 0, 0, 101),
            (1, 3, 0, 0, 101),
            (1, 2, 101, 0, 102),
            (1, 2, 0, 101, 102),
            (1, 2, 0, 0, 99),
        ] {
            assert!(check_cursor(&s, tuple.0, tuple.1, tuple.2, tuple.3, tuple.4).is_err());
        }
    }
    #[test]
    fn parent_rename_and_delete_update_graph() {
        let (_dir, conn) = fixture();
        upsert(&conn, &rec(10, 5, "parent", 10, 0)).unwrap();
        upsert(&conn, &rec(11, 10, "file", 11, 0)).unwrap();
        apply(&conn, &rec(10, 5, "new", 12, RENAME_NEW)).unwrap();
        assert_eq!(
            resolve("000000000000000b", &graph(&conn).unwrap()).unwrap(),
            "new\\file"
        );
        apply(&conn, &rec(11, 10, "file", 13, DELETE)).unwrap();
        assert_eq!(graph(&conn).unwrap().len(), 1);
    }
    #[test]
    fn older_replay_does_not_override_newer_mft_record() {
        let (_dir, conn) = fixture();
        upsert(&conn, &rec(10, 5, "new", 100, 0)).unwrap();
        apply(&conn, &rec(10, 5, "old", 90, 0)).unwrap();
        apply(&conn, &rec(10, 5, "old", 95, DELETE)).unwrap();
        assert_eq!(
            resolve("000000000000000a", &graph(&conn).unwrap()).unwrap(),
            "new"
        );
    }
    #[test]
    fn failed_batch_rolls_back_records_and_cursor() {
        let (_dir, mut conn) = fixture();
        let state = State {
            volume: "E:".into(),
            volume_guid: "synthetic-volume".into(),
            serial: 1,
            journal_id: 2,
            next_usn: 100,
        };
        save(&conn, &state).unwrap();
        {
            let tx = conn.transaction().unwrap();
            apply(&tx, &rec(10, 5, "new", 101, 0)).unwrap();
            let mut updated = state.clone();
            updated.next_usn = 200;
            save(&tx, &updated).unwrap();
            assert!(apply(&tx, &rec(11, 5, "link", 102, HARD_LINK)).is_err());
        }
        assert!(graph(&conn).unwrap().is_empty());
        assert_eq!(load(&conn).unwrap().next_usn, 100);
    }
    #[test]
    fn cycle_and_orphan_are_unresolved() {
        let (_dir, conn) = fixture();
        upsert(&conn, &rec(10, 11, "a", 1, 0)).unwrap();
        upsert(&conn, &rec(11, 10, "b", 1, 0)).unwrap();
        upsert(&conn, &rec(12, 13, "c", 1, 0)).unwrap();
        let g = graph(&conn).unwrap();
        assert!(resolve("000000000000000a", &g).is_none());
        assert!(resolve("000000000000000c", &g).is_none());
    }
    #[test]
    fn database_failure_keeps_last_committed_generation() {
        let (_dir, mut conn) = fixture();
        let state = State {
            volume: "E:".into(),
            volume_guid: "synthetic".into(),
            serial: 1,
            journal_id: 2,
            next_usn: 100,
        };
        save(&conn, &state).unwrap();
        upsert(&conn, &rec(10, 5, "old", 90, 0)).unwrap();
        conn.execute_batch("CREATE TRIGGER inject_failure BEFORE INSERT ON records WHEN NEW.name='fail' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        {
            let tx = conn.transaction().unwrap();
            tx.execute("DELETE FROM records", []).unwrap();
            let mut updated = state.clone();
            updated.next_usn = 200;
            save(&tx, &updated).unwrap();
            assert!(upsert(&tx, &rec(11, 5, "fail", 190, 0)).is_err());
        }
        assert_eq!(load(&conn).unwrap().next_usn, 100);
        assert_eq!(
            resolve("000000000000000a", &graph(&conn).unwrap()).as_deref(),
            Some("old")
        );
    }
    #[test]
    fn unrelated_database_is_not_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("other.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE user_data(value TEXT); INSERT INTO user_data VALUES('keep');",
            )
            .unwrap();
        }
        assert!(open(&path, true).is_err());
        let c = Connection::open(&path).unwrap();
        assert_eq!(
            c.query_row("SELECT value FROM user_data", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "keep"
        );
    }
}
