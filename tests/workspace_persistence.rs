use dao_shell::workspace::{Message, Session, Workspace, WorkspaceStore};
use std::fs;

#[test]
fn malformed_and_complete_future_schema_remain_unchanged_and_block_saves() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut future = serde_json::to_value(Workspace::default()).unwrap();
    future["version"] = 99.into();
    for bytes in [b"{broken".to_vec(), serde_json::to_vec(&future).unwrap()] {
        fs::write(&path, &bytes).unwrap();
        let mut store = WorkspaceStore::new(path.clone());
        assert!(store.load().is_err());
        assert!(store.save(&Workspace::default()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn total_size_and_nested_authority_are_rejected_without_replacing_valid_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut store = WorkspaceStore::new(path.clone());
    let mut value = store.load().unwrap();
    store.save(&value).unwrap();
    let original = fs::read(&path).unwrap();
    value.sessions.push(Session {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Synthetic history".into(),
        draft: String::new(),
        messages: (0..40)
            .map(|_| Message {
                role: "assistant".into(),
                text: "x".repeat(256000),
                error: false,
            })
            .collect(),
    });
    assert!(value.validate().is_ok());
    assert!(store.save(&value).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    value.sessions[0].messages.clear();
    let mut encoded = serde_json::to_value(&value).unwrap();
    encoded["sessions"][0]["confirmation"] = "forbidden".into();
    assert!(serde_json::from_value::<Workspace>(encoded).is_err());
    value.sessions[0].messages.push(Message {
        role: "tool".into(),
        text: "raw protocol".into(),
        error: false,
    });
    assert!(store.save(&value).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn invalid_tabs_duplicate_sessions_and_excess_messages_cannot_replace_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut store = WorkspaceStore::new(path.clone());
    let mut value = store.load().unwrap();
    store.save(&value).unwrap();
    let original = fs::read(&path).unwrap();
    value.tabs.push("missing".into());
    assert!(store.save(&value).is_err());
    value.tabs = vec!["home".into(), "home".into()];
    assert!(store.save(&value).is_err());
    value.tabs = vec!["home".into()];
    let session = Session {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Fixture".into(),
        draft: String::new(),
        messages: vec![],
    };
    value.sessions = vec![session.clone(), session];
    assert!(store.save(&value).is_err());
    value.sessions.pop();
    value.sessions[0].messages = vec![
        Message {
            role: "user".into(),
            text: "fixture".into(),
            error: false,
        };
        2001
    ];
    assert!(store.save(&value).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}
