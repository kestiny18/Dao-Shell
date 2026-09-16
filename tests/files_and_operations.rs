use dao_shell::{
    core::{
        Cancellation, DirectoryEffect, FileIdentity, MoveItem, Operation, Status, now, safe_text,
    },
    files::{self, Objects, Scope, Search, Sort},
    storage::Journal,
};
#[cfg(windows)]
use dao_shell::{operations, platform};
use std::fs;

#[test]
fn search_is_scoped_filtered_paged_and_references_are_real() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("scope");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("合同甲.pdf"), "abcd").unwrap();
    fs::write(root.join("合同乙.pdf"), "abcdef").unwrap();
    fs::write(root.join("合同草稿.txt"), "x").unwrap();
    fs::write(temp.path().join("合同外部.pdf"), "out").unwrap();
    let scope = Scope::new(std::slice::from_ref(&root), &[]).unwrap();
    let mut objects = Objects::default();
    let request = Search {
        query: "合同".into(),
        extension: Some("PDF".into()),
        sort: Sort::SizeDesc,
        limit: Some(1),
        ..Default::default()
    };
    let first = files::search(&request, &scope, &mut objects, &Cancellation::default()).unwrap();
    assert_eq!(first.matched_in_scan, 2);
    assert!(first.has_more);
    assert!(!first.truncated);
    assert!(first.items[0].path.ends_with("合同乙.pdf"));
    objects.checked(&first.items[0].id, &scope, false).unwrap();
    assert!(objects.checked(&first.items[0].id, &scope, true).is_err());
    let second = files::search(
        &Search { page: 1, ..request },
        &scope,
        &mut objects,
        &Cancellation::default(),
    )
    .unwrap();
    assert!(second.items[0].path.ends_with("合同甲.pdf"));
    assert!(!second.has_more);
    assert!(scope.check(temp.path(), false).is_err());
    assert!(objects.get("invented-id").is_err());
}

#[test]
fn scope_rejects_parent_traversal_and_prefix_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let allowed = temp.path().join("allowed");
    let sibling = temp.path().join("allowed-secret");
    fs::create_dir(&allowed).unwrap();
    fs::create_dir(&sibling).unwrap();
    let scope = Scope::new(
        std::slice::from_ref(&allowed),
        std::slice::from_ref(&allowed),
    )
    .unwrap();
    assert!(scope.check(&sibling, false).is_err());
    assert!(
        scope
            .destination(&allowed.join("..").join("allowed-secret"))
            .is_err()
    );
    #[cfg(windows)]
    for name in ["NUL", "CON.txt", "data.txt:stream", "dir.", "dir "] {
        assert!(scope.destination(&allowed.join(name)).is_err(), "{name}");
    }
}

#[test]
fn cancellation_prevents_search_and_invalid_arguments_fail() {
    let temp = tempfile::tempdir().unwrap();
    let scope = Scope::new(&[temp.path().into()], &[]).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(files::search(&Search::default(), &scope, &mut Objects::default(), &cancel).is_err());
    assert!(
        files::search(
            &Search {
                limit: Some(51),
                ..Default::default()
            },
            &scope,
            &mut Objects::default(),
            &Cancellation::default()
        )
        .is_err()
    );
    assert!(serde_json::from_value::<Search>(serde_json::json!({"approved":true})).is_err());
}

#[test]
fn journal_is_exclusive_and_terminal_output_cannot_control_the_prompt() {
    let temp = tempfile::tempdir().unwrap();
    let first = Journal::open(temp.path()).unwrap();
    assert!(Journal::open(temp.path()).is_err());
    drop(first);
    Journal::open(temp.path()).unwrap();
    assert_eq!(
        safe_text("a\x1b[2J\r\ny\u{202e}"),
        "a\\u{1b}[2J\\u{d}\\u{a}y\\u{202e}"
    );
}

fn operation_with(statuses: &[Status], directory: Option<Status>) -> Operation {
    let identity = FileIdentity {
        volume: 1,
        index: 1,
        size: 4,
        modified: 5,
        directory: false,
    };
    Operation {
        id: "test".into(),
        created_at: now(),
        status: Status::Executing,
        confirmed_at: Some(now()),
        destination_anchor: "anchor".into(),
        destination_identity: identity.clone(),
        items: statuses
            .iter()
            .map(|s| MoveItem {
                source: "source".into(),
                destination: "dest".into(),
                identity: identity.clone(),
                status: *s,
                evidence: String::new(),
            })
            .collect(),
        directories: directory
            .map(|s| {
                vec![DirectoryEffect {
                    path: "created".into(),
                    status: s,
                    identity: None,
                }]
            })
            .unwrap_or_default(),
    }
}
#[test]
fn unknown_and_directory_effects_are_not_hidden_by_batch_status() {
    assert_eq!(
        operation_with(
            &[Status::Succeeded, Status::Unknown, Status::NotStarted],
            None
        )
        .aggregate(false),
        Status::Unknown
    );
    assert_eq!(
        operation_with(&[Status::Succeeded, Status::Failed], None).aggregate(false),
        Status::Partial
    );
    assert_eq!(
        operation_with(&[Status::NotStarted], Some(Status::Succeeded)).aggregate(true),
        Status::Partial
    );
    assert_eq!(
        operation_with(&[Status::NotStarted], None).aggregate(true),
        Status::Cancelled
    );
}

#[cfg(windows)]
mod windows {
    use super::*;
    struct Fixture {
        _temp: tempfile::TempDir,
        root: std::path::PathBuf,
        scope: Scope,
        objects: Objects,
        id: String,
        journal: Journal,
    }
    fn setup() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("合同.txt"), "verified content").unwrap();
        let scope = Scope::new(std::slice::from_ref(&root), std::slice::from_ref(&root)).unwrap();
        let mut objects = Objects::default();
        let id = objects
            .insert(&scope.check(&root.join("合同.txt"), false).unwrap())
            .unwrap()
            .id;
        let journal = Journal::open(&temp.path().join("state")).unwrap();
        Fixture {
            _temp: temp,
            root,
            scope,
            objects,
            id,
            journal,
        }
    }
    #[test]
    fn confirmed_move_creates_directory_preserves_identity_and_persists_receipt() {
        let mut f = setup();
        let original = f.objects.get(&f.id).unwrap().identity.clone();
        let plan = operations::prepare(
            &[f.id],
            &f.root.join("归档"),
            &f.scope,
            &f.objects,
            &mut f.journal,
        )
        .unwrap();
        assert!(!f.root.join("归档").exists());
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |p| {
                assert_eq!(p.items.len(), 1);
                assert_eq!(p.directories.len(), 1);
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Succeeded, "{receipt:#?}");
        assert!(!f.root.join("合同.txt").exists());
        assert_eq!(
            fs::read_to_string(f.root.join("归档\\合同.txt")).unwrap(),
            "verified content"
        );
        assert!(
            platform::identity(&f.root.join("归档\\合同.txt"))
                .unwrap()
                .same_object(&original)
        );
        assert_eq!(f.journal.list().unwrap()[0].status, Status::Succeeded);
    }
    #[test]
    fn rejected_confirmation_creates_nothing() {
        let mut f = setup();
        let plan = operations::prepare(
            &[f.id],
            &f.root.join("new"),
            &f.scope,
            &f.objects,
            &mut f.journal,
        )
        .unwrap();
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |_| Ok(false),
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Cancelled);
        assert!(!f.root.join("new").exists());
        assert!(f.root.join("合同.txt").exists());
    }
    #[test]
    fn changed_source_invalidates_confirmation_even_with_same_filename() {
        let mut f = setup();
        let plan = operations::prepare(
            &[f.id],
            &f.root.join("new"),
            &f.scope,
            &f.objects,
            &mut f.journal,
        )
        .unwrap();
        let source = f.root.join("合同.txt");
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |_| {
                fs::rename(&source, f.root.join("old.txt"))?;
                fs::write(&source, "replacement")?;
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Invalidated);
        assert!(!f.root.join("new").exists());
    }
    #[test]
    fn destination_appearing_during_confirmation_never_gets_overwritten() {
        let mut f = setup();
        let dest = f.root.join("archive");
        fs::create_dir(&dest).unwrap();
        let plan =
            operations::prepare(&[f.id], &dest, &f.scope, &f.objects, &mut f.journal).unwrap();
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |_| {
                fs::write(dest.join("合同.txt"), "keep me")?;
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Invalidated);
        assert_eq!(
            fs::read_to_string(dest.join("合同.txt")).unwrap(),
            "keep me"
        );
    }
    #[test]
    fn native_no_replace_and_pinned_parent_guards_hold() {
        let f = setup();
        let source = f.scope.check(&f.root.join("合同.txt"), false).unwrap();
        let target = f.root.join("occupied.txt");
        fs::write(&target, "keep").unwrap();
        assert!(
            platform::move_file(&source, &target, &f.objects.get(&f.id).unwrap().identity).is_err()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "keep");
        let guard = platform::pin_directory(&f.scope.check(&f.root, false).unwrap()).unwrap();
        assert!(fs::rename(&f.root, f.root.with_file_name("swapped")).is_err());
        drop(guard);
    }
    #[test]
    fn cancelled_before_execute_never_starts_a_write() {
        let mut f = setup();
        let cancel = Cancellation::default();
        let plan = operations::prepare(
            &[f.id],
            &f.root.join("new"),
            &f.scope,
            &f.objects,
            &mut f.journal,
        )
        .unwrap();
        let receipt = operations::run(plan, &f.scope, &mut f.journal, &cancel, |_| {
            cancel.cancel();
            Ok(true)
        })
        .unwrap();
        assert_eq!(receipt.status, Status::Cancelled);
        assert!(!f.root.join("new").exists());
    }
    #[test]
    fn restart_reconciles_an_effect_without_replaying_it() {
        let mut f = setup();
        let dest = f.root.join("archive");
        fs::create_dir(&dest).unwrap();
        let mut plan =
            operations::prepare(&[f.id], &dest, &f.scope, &f.objects, &mut f.journal).unwrap();
        plan.status = Status::Executing;
        plan.confirmed_at = Some(now());
        plan.items[0].status = Status::Executing;
        f.journal.save(&plan).unwrap();
        let item = &plan.items[0];
        platform::move_file(&item.source, &item.destination, &item.identity).unwrap();
        let receipts = operations::reconcile(&mut f.journal).unwrap();
        assert_eq!(receipts[0].status, Status::Succeeded);
        assert!(dest.join("合同.txt").exists());
        assert!(operations::reconcile(&mut f.journal).unwrap().is_empty());
    }
    #[test]
    fn unresolved_directory_stays_unknown_and_blocks_the_same_object() {
        let mut f = setup();
        let dest = f.root.join("archive");
        let mut plan =
            operations::prepare(&[f.id.clone()], &dest, &f.scope, &f.objects, &mut f.journal)
                .unwrap();
        plan.status = Status::Executing;
        plan.confirmed_at = Some(now());
        plan.directories[0].status = Status::Executing;
        f.journal.save(&plan).unwrap();
        fs::create_dir(&dest).unwrap();
        assert_eq!(
            operations::reconcile(&mut f.journal).unwrap()[0].status,
            Status::Unknown
        );
        assert!(operations::prepare(&[f.id], &dest, &f.scope, &f.objects, &mut f.journal).is_err());
    }
    #[test]
    fn two_hard_links_cannot_duplicate_a_single_object_in_a_batch() {
        let mut f = setup();
        let alias = f.root.join("alias.txt");
        fs::hard_link(f.root.join("合同.txt"), &alias).unwrap();
        let alias = f
            .objects
            .insert(&f.scope.check(&alias, false).unwrap())
            .unwrap();
        assert!(
            operations::prepare(
                &[f.id, alias.id],
                &f.root.join("archive"),
                &f.scope,
                &f.objects,
                &mut f.journal
            )
            .is_err()
        );
    }

    #[test]
    fn replaced_destination_directory_invalidates_the_plan() {
        let mut f = setup();
        let destination = f.root.join("archive");
        fs::create_dir(&destination).unwrap();
        let plan = operations::prepare(&[f.id], &destination, &f.scope, &f.objects, &mut f.journal)
            .unwrap();
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |_| {
                fs::rename(&destination, f.root.join("old-archive"))?;
                fs::create_dir(&destination)?;
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Invalidated);
        assert!(f.root.join("合同.txt").exists());
        assert!(!destination.join("合同.txt").exists());
    }

    #[test]
    fn failure_after_first_move_reports_partial_and_leaves_later_items_untouched() {
        use std::os::windows::fs::OpenOptionsExt;
        let mut f = setup();
        let second = f.root.join("second.txt");
        let third = f.root.join("third.txt");
        fs::write(&second, "second").unwrap();
        fs::write(&third, "third").unwrap();
        let second_id = f
            .objects
            .insert(&f.scope.check(&second, false).unwrap())
            .unwrap()
            .id;
        let third_id = f
            .objects
            .insert(&f.scope.check(&third, false).unwrap())
            .unwrap()
            .id;
        let destination = f.root.join("archive");
        fs::create_dir(&destination).unwrap();
        let plan = operations::prepare(
            &[f.id, second_id, third_id],
            &destination,
            &f.scope,
            &f.objects,
            &mut f.journal,
        )
        .unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&second)
            .unwrap();
        let receipt = operations::run(
            plan,
            &f.scope,
            &mut f.journal,
            &Cancellation::default(),
            |_| Ok(true),
        )
        .unwrap();
        assert_eq!(receipt.status, Status::Partial, "{receipt:#?}");
        assert_eq!(
            receipt.items.iter().map(|i| i.status).collect::<Vec<_>>(),
            [Status::Succeeded, Status::Failed, Status::NotStarted]
        );
        assert!(destination.join("合同.txt").exists());
        assert!(second.exists() && third.exists());
        drop(lock);
    }
}
