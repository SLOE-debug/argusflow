//! SQLite 配置持久化和正式录制包读取的集成边界。
use argusflow_ai::{AiConfig, ConfigStore, Evidence, SaveConfig};
use argusflow_recorder::{Journal, RecordData, Session, SessionPhase};
#[test]
fn sqlite_preserves_updates_without_returning_secret() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.sqlite3");
    let mut store = ConfigStore::open(&path).unwrap();
    assert!(!store.view().unwrap().has_key);
    let config = AiConfig::default();
    let view = store
        .save(SaveConfig {
            config: config.clone(),
            api_key: Some("test-secret".into()),
        })
        .unwrap();
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("test-secret")
    );
    drop(store);
    let mut store = ConfigStore::open(&path).unwrap();
    assert_eq!(store.load().unwrap().1, "test-secret");
    let mut changed = config.clone();
    changed.model = "other-model".into();
    store
        .save(SaveConfig {
            config: changed.clone(),
            api_key: None,
        })
        .unwrap();
    assert_eq!(store.load().unwrap().1, "test-secret");
    changed.endpoint = "https://other.example/chat/completions".into();
    assert!(
        store
            .save(SaveConfig {
                config: changed,
                api_key: None
            })
            .is_err()
    );
    assert_eq!(store.view().unwrap().config.endpoint, config.endpoint);
    store
        .save(SaveConfig {
            config,
            api_key: Some(String::new()),
        })
        .unwrap();
    assert!(!store.view().unwrap().has_key);
}
#[test]
fn invalid_config_never_replaces_saved_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = ConfigStore::open(&temp.path().join("db")).unwrap();
    let config = AiConfig {
        max_rounds: 0,
        ..Default::default()
    };
    assert!(
        store
            .save(SaveConfig {
                config,
                api_key: Some("x".into())
            })
            .is_err()
    );
    assert!(!store.view().unwrap().has_key);
}
#[test]
fn active_or_corrupt_recordings_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recording");
    let session = Session {
        id: "test".into(),
        format: 2,
        created_ms: 0,
        qpc_origin: 0,
        qpc_frequency: 1000,
        capture_session: None,
        policy: "test".into(),
    };
    let mut journal = Journal::create(&path, &session).unwrap();
    assert!(Evidence::load(&path).is_err());
    journal
        .append(
            RecordData::State {
                phase: SessionPhase::Stopped,
                reason: "finished".into(),
            },
            0,
        )
        .unwrap();
    journal.sync().unwrap();
    assert!(Evidence::load(&path).is_ok());
    drop(journal);
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path.join("events.afr"))
        .unwrap()
        .write_all(b"broken")
        .unwrap();
    assert!(Evidence::load(&path).is_err());
}
