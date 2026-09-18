use dao_shell::{
    core::Cancellation,
    files::{self, Kind, MatchMode, Objects, Scope, Search},
};
use std::fs;

#[test]
fn cli_numbered_shortcuts_do_not_reuse_results_after_an_empty_search() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("fixture.unsupported-test-extension");
    fs::write(&file, "fixture").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_daosh"))
        .arg("--config")
        .arg(temp.path().join("config.json"))
        .arg("--data-dir")
        .arg(temp.path().join("state"))
        .arg("--read-root")
        .arg(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/search fixture\n/search no-such-file\n/open 1\n/move 1 nowhere\n/quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(text.matches("序号不在最近一次结果中").count(), 2);
    assert!(file.exists());
}

#[test]
fn compact_roots_preserve_read_write_boundaries_and_prefix_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("project");
    let child = parent.join("docs");
    let sibling = temp.path().join("project-other");
    fs::create_dir_all(&child).unwrap();
    fs::create_dir(&sibling).unwrap();
    let scope = Scope::new(
        &[child.clone(), parent.clone(), sibling.clone()],
        std::slice::from_ref(&child),
    )
    .unwrap();
    assert_eq!(scope.read_roots().len(), 2);
    assert_eq!(scope.write_roots().len(), 1);
    assert!(scope.check(&parent, true).is_err());
    assert!(scope.check(&sibling, true).is_err());
    assert!(scope.check(&child, true).is_ok());
    let compact = Scope::new(
        std::slice::from_ref(&child),
        &[child.clone(), parent.clone(), parent],
    )
    .unwrap();
    assert_eq!(compact.read_roots().len(), 1);
    assert_eq!(compact.write_roots().len(), 1);
}

#[test]
fn path_terms_find_initialization_script_in_one_search_and_rank_local_matches() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let near = root.join("sample-project/客户端初始化");
    let far = near.join("old/nested");
    fs::create_dir_all(&far).unwrap();
    fs::write(root.join("客户端初始化.sql"), "name match").unwrap();
    fs::write(near.join("dump-client.sql"), "parent match").unwrap();
    fs::write(far.join("backup.sql"), "ancestor match").unwrap();
    fs::write(near.join("notes.txt"), "wrong type").unwrap();
    fs::create_dir(near.join("not-a-file.sql")).unwrap();
    let scope = Scope::new(std::slice::from_ref(&root), &[]).unwrap();
    let request = Search {
        terms: vec!["客户端".into(), "初始化".into()],
        extension: Some("sql".into()),
        kind: Kind::File,
        ..Default::default()
    };
    let result = files::search(
        &request,
        &scope,
        &mut Objects::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(result.items.len(), 3);
    assert!(result.items[0].path.ends_with("客户端初始化.sql"));
    assert!(result.items[1].path.ends_with("dump-client.sql"));
    assert!(result.items[2].path.ends_with("backup.sql"));
    assert!(!result.truncated);

    let any = Search {
        terms: vec!["no-match".into(), "DUMP".into()],
        match_mode: MatchMode::Any,
        ..Default::default()
    };
    assert_eq!(
        files::search(
            &any,
            &scope,
            &mut Objects::default(),
            &Cancellation::default()
        )
        .unwrap()
        .items
        .len(),
        1
    );
    let all = Search {
        match_mode: MatchMode::All,
        ..any
    };
    assert!(
        files::search(
            &all,
            &scope,
            &mut Objects::default(),
            &Cancellation::default()
        )
        .unwrap()
        .items
        .is_empty()
    );
    // Ancestors outside the configured root are never part of a query match.
    let outside = Search {
        terms: vec![
            temp.path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ],
        ..Default::default()
    };
    assert!(
        files::search(
            &outside,
            &scope,
            &mut Objects::default(),
            &Cancellation::default()
        )
        .unwrap()
        .items
        .is_empty()
    );
}

#[test]
fn repeated_discovery_reuses_id_but_changes_do_not_refresh_old_references() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("sample.txt");
    fs::write(&file, "before").unwrap();
    let scope = Scope::new(&[temp.path().to_owned()], &[]).unwrap();
    let mut objects = Objects::default();
    let first = objects.insert(&file).unwrap();
    for _ in 0..2001 {
        assert_eq!(objects.insert(&file).unwrap().id, first.id);
    }
    fs::write(&file, "different length after edit").unwrap();
    let changed = objects.insert(&file).unwrap();
    assert_ne!(changed.id, first.id);
    assert!(objects.checked(&first.id, &scope, false).is_err());
    assert!(objects.checked(&changed.id, &scope, false).is_ok());
    fs::rename(&file, temp.path().join("old.txt")).unwrap();
    fs::write(&file, "replacement").unwrap();
    let replaced = objects.insert(&file).unwrap();
    assert_ne!(replaced.id, changed.id);
    assert!(objects.checked(&changed.id, &scope, false).is_err());
}
