//! Independent Windows temporary fixtures; never reads the real Startup folder.
#![cfg(windows)]
use dao_shell::{
    application_actions::journal::Journal,
    core::Cancellation,
    software::{engine::Engine, startup::PinnedShortcut},
};
use serde_json::{Value, json};
use std::{
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, Value) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("Startup");
    let retained = d.path().join("DaoShellDisabledStartup");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&retained).unwrap();
    std::fs::write(retained.join(".dao-owner"), b"dao-shell-startup-v1").unwrap();
    let original = root.join("fixture.lnk");
    let mut bytes = vec![0; 76];
    bytes[..4].copy_from_slice(&[76, 0, 0, 0]);
    bytes[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
    std::fs::write(&original, bytes).unwrap();
    let p = PinnedShortcut::inspect(&original).unwrap();
    let proof = json!({"original":original,"retained":retained.join("saved.lnk"),"identity":p.identity,"digest":p.digest.to_vec()});
    p.relocate(Path::new(proof["retained"].as_str().unwrap()))
        .unwrap();
    (d, root, retained, proof)
}
fn seed(dir: &Path, operation: &str, lifecycle: &str, proof: &Value) {
    Journal::open(dir).unwrap().save(&json!({"operation_id":"interrupted","created_at":"2020-01-01T00:00:00Z","operation":operation,"lifecycle":lifecycle,"startup_proof":proof})).unwrap();
}
#[test]
fn restore_exit_child() {
    let Some(dir) = std::env::var_os("DAOSHELL_INDEPENDENT_RESTORE_EXIT") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let proof: Value =
        serde_json::from_slice(&std::fs::read(dir.join("proof.json")).unwrap()).unwrap();
    let p = PinnedShortcut::inspect(Path::new(proof["retained"].as_str().unwrap())).unwrap();
    Engine::open(&dir.join("journal"))
        .unwrap()
        .local_transition(
            json!({"operation_id":"restore","operation":"startup_restore","startup_proof":proof}),
            &Cancellation::default(),
            &mut |_| Ok(true),
            &mut || {
                p.relocate(Path::new(proof["original"].as_str().unwrap()))?;
                std::process::exit(93);
            },
        )
        .unwrap();
    panic!("child failed to exit");
}
#[test]
fn interrupted_restore_process_is_observed_without_replay_or_success_attribution() {
    let (d, root, retained, proof) = fixture();
    std::fs::write(
        d.path().join("proof.json"),
        serde_json::to_vec(&proof).unwrap(),
    )
    .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "restore_exit_child", "--nocapture"])
        .env("DAOSHELL_INDEPENDENT_RESTORE_EXIT", d.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(93));
    let mut e = Engine::open(&d.path().join("journal")).unwrap();
    assert_eq!(e.history().unwrap()[0]["lifecycle"], "unknown");
    assert_eq!(e.history().unwrap()[0]["startup_proof"], proof);
    assert!(Path::new(proof["original"].as_str().unwrap()).exists());
    assert!(!Path::new(proof["retained"].as_str().unwrap()).exists());
    let rows = e.check_startup(&root, &retained).unwrap();
    assert_eq!(rows[0]["lifecycle"], "startup_checked_original");
    assert_eq!(rows[0]["startup_check"]["attribution"], "unknown");
    assert_eq!(rows[0]["startup_check"]["automatic_replay"], false);
    assert!(!e.unresolved().unwrap());
}
#[test]
fn io_sharing_denial_and_foreign_retention_marker_keep_gate_closed() {
    let (d, root, retained, proof) = fixture();
    seed(
        &d.path().join("journal"),
        "startup_disable",
        "dispatching",
        &proof,
    );
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(proof["retained"].as_str().unwrap())
        .unwrap();
    let mut e = Engine::open(&d.path().join("journal")).unwrap();
    assert_eq!(
        e.check_startup(&root, &retained).unwrap()[0]["lifecycle"],
        "unknown"
    );
    assert!(e.unresolved().unwrap());
    drop(lock);
    std::fs::write(retained.join(".dao-owner"), b"foreign-owner").unwrap();
    assert_eq!(
        e.check_startup(&root, &retained).unwrap()[0]["lifecycle"],
        "unknown"
    );
    assert!(e.unresolved().unwrap());
    std::fs::write(retained.join(".dao-owner"), b"dao-shell-startup-v1").unwrap();
    assert_eq!(
        e.check_startup(&root, &retained).unwrap()[0]["lifecycle"],
        "startup_checked_retained"
    );
    assert!(!e.unresolved().unwrap());
}
#[test]
fn checked_retained_restore_proof_survives_automatic_and_explicit_pruning() {
    let (d, root, retained, proof) = fixture();
    seed(
        &d.path().join("journal"),
        "startup_restore",
        "unknown",
        &proof,
    );
    {
        let mut e = Engine::open(&d.path().join("journal")).unwrap();
        assert_eq!(
            e.check_startup(&root, &retained).unwrap()[0]["lifecycle"],
            "startup_checked_retained"
        );
    }
    let mut j = Journal::open(&d.path().join("journal")).unwrap();
    assert_eq!(j.prune(true).unwrap(), 0);
    assert_eq!(j.list().unwrap()[0]["startup_proof"], proof);
}
#[test]
fn sqlite_final_update_failure_keeps_pre_move_proof_for_restore_recovery() {
    let (d, root, retained, proof) = fixture();
    let journal = d.path().join("journal");
    let mut e = Engine::open(&journal).unwrap();
    let p = PinnedShortcut::inspect(Path::new(proof["retained"].as_str().unwrap())).unwrap();
    let r = e.local_transition(json!({"operation_id":"restore","operation":"startup_restore","startup_proof":proof}), &Cancellation::default(), &mut |_| Ok(true), &mut || {
        // A SQLite write failure after the native move, independent of journal size limits.
        let db = rusqlite::Connection::open(journal.join("actions.db"))?;
        db.execute_batch("CREATE TRIGGER independent_fail_update BEFORE UPDATE ON app_actions BEGIN SELECT RAISE(FAIL, 'independent final write failure'); END;")?;
        p.relocate(Path::new(proof["original"].as_str().unwrap()))?;
        Ok(json!({"lifecycle":"verified"}))
    }).unwrap();
    assert_eq!(r["persistence_warning"], true);
    assert_eq!(r["lifecycle"], "unknown");
    assert_eq!(e.history().unwrap()[0]["lifecycle"], "dispatching");
    assert_eq!(e.history().unwrap()[0]["startup_proof"], proof);
    drop(e);
    rusqlite::Connection::open(journal.join("actions.db"))
        .unwrap()
        .execute_batch("DROP TRIGGER independent_fail_update;")
        .unwrap();
    let mut reopened = Engine::open(&journal).unwrap();
    assert!(reopened.unresolved().unwrap());
    assert_eq!(
        reopened.check_startup(&root, &retained).unwrap()[0]["lifecycle"],
        "startup_checked_original"
    );
    assert!(!reopened.unresolved().unwrap());
}
