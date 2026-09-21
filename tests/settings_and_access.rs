use dao_shell::{
    config::{AccessMode, Config},
    core::Cancellation,
    files::{Objects, Search},
    model::provider::{Connection, ModelChoice},
    runtime::{Interaction, Runtime},
    settings::{self, KeyUpdate, SettingsUpdate},
};
use serde_json::json;
use std::fs;

#[test]
fn legacy_settings_migrate_and_stale_saves_preserve_current_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    fs::write(&path, r#"{"read_roots":[],"write_roots":[],"model":{"endpoint":"http://127.0.0.1:10001/v1/chat/completions","model":"fixture","api_key_env":""}}"#).unwrap();
    let view = settings::read(&path).unwrap();
    assert_eq!(view.config.access_mode, AccessMode::Restricted);
    assert_eq!(
        view.config.models.resolve("legacy").unwrap().model,
        "fixture"
    );
    let mut changed = view.config.clone();
    changed.access_mode = AccessMode::Full;
    let saved = settings::save(
        &path,
        SettingsUpdate {
            expected_revision: view.revision.clone(),
            config: changed,
            keys: vec![],
        },
    )
    .unwrap();
    assert_eq!(saved.config.access_mode, AccessMode::Full);
    let before = fs::read(&path).unwrap();
    assert!(
        settings::save(
            &path,
            SettingsUpdate {
                expected_revision: view.revision,
                config: view.config,
                keys: vec![]
            }
        )
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn multiple_connections_keep_keys_out_of_views_and_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    let mut view = settings::read(&path).unwrap();
    let unique = uuid::Uuid::new_v4().simple().to_string();
    for id in ["first", "second"] {
        view.config.models.connections.push(Connection {
            id: id.into(),
            label: id.into(),
            endpoint: format!("https://example.invalid/{unique}/{id}/chat/completions"),
            api_key_env: format!("DAO_TEST_{unique}_{id}"),
        });
        view.config.models.choices.push(ModelChoice {
            id: id.into(),
            connection_id: id.into(),
            name: format!("model-{id}"),
        });
    }
    view.config.models.active = Some("second".into());
    let secret = "synthetic-settings-secret";
    let saved = settings::save(
        &path,
        SettingsUpdate {
            expected_revision: view.revision,
            config: view.config,
            keys: vec![KeyUpdate {
                connection_id: "second".into(),
                key: secret.into(),
                persist: false,
            }],
        },
    )
    .unwrap();
    assert_eq!(saved.config.model.as_ref().unwrap().model, "model-second");
    assert_eq!(saved.credential_available, vec!["second"]);
    assert!(!serde_json::to_string(&saved).unwrap().contains(secret));
    assert!(!fs::read_to_string(&path).unwrap().contains(secret));
    let mut invalid = saved.config.clone();
    invalid.models.choices[0].connection_id = "missing".into();
    let before = fs::read(&path).unwrap();
    assert!(
        settings::save(
            &path,
            SettingsUpdate {
                expected_revision: saved.revision,
                config: invalid,
                keys: vec![]
            }
        )
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn full_scope_separates_default_search_from_authority_and_restricts_writes() {
    let temp = tempfile::tempdir().unwrap();
    let preferred = temp.path().join("preferred");
    let outside = temp.path().join("outside");
    fs::create_dir(&preferred).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("outside.txt"), "fixture").unwrap();
    let mut config = Config {
        read_roots: vec![preferred.clone()],
        ..Default::default()
    };
    assert!(config.scope(false).unwrap().check(&outside, false).is_err());
    config.access_mode = AccessMode::Full;
    let scope = config.scope(false).unwrap();
    assert!(scope.check(&outside, false).is_ok());
    assert!(scope.check(&outside, true).is_err());
    assert!(config.scope(true).unwrap().check(&outside, true).is_ok());
    let mut objects = Objects::default();
    let cancel = Cancellation::default();
    let default = dao_shell::files::search(
        &Search {
            query: "outside.txt".into(),
            ..Default::default()
        },
        &scope,
        &mut objects,
        &cancel,
    )
    .unwrap();
    assert!(default.items.is_empty());
    let explicit = dao_shell::files::search(
        &Search {
            query: "outside.txt".into(),
            directory: Some(outside),
            ..Default::default()
        },
        &scope,
        &mut objects,
        &cancel,
    )
    .unwrap();
    assert_eq!(explicit.items.len(), 1);
    config.access_mode = AccessMode::Restricted;
    assert!(
        config
            .scope(false)
            .unwrap()
            .check(&explicit.items[0].path, false)
            .is_err()
    );
}

#[test]
fn full_authority_cannot_move_without_a_durable_journal() {
    struct Ui;
    impl Interaction for Ui {
        fn confirm_move(&mut self, _: &dao_shell::core::Operation) -> anyhow::Result<bool> {
            panic!("must reject before confirmation")
        }
        fn confirm_open(&mut self, _: &dao_shell::core::FileObject) -> anyhow::Result<bool> {
            panic!("unexpected open")
        }
        fn progress(&mut self, _: &str) {}
        fn result(&mut self, _: &str, _: &serde_json::Value) {}
    }
    let temp = tempfile::tempdir().unwrap();
    let mut runtime = Runtime {
        scope: dao_shell::files::Scope::full(&[temp.path().into()], true).unwrap(),
        objects: Objects::default(),
        journal: None,
        cancel: Cancellation::default(),
        last_results: vec![],
        side_effects_blocked: false,
    };
    let error = runtime
        .call(
            "file_move_batch",
            json!({"object_ids":[],"destination":temp.path()}),
            &mut Ui,
        )
        .unwrap_err();
    assert!(error.to_string().contains("持久写操作记录"));
}
