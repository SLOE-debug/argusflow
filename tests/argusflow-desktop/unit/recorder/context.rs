use super::*;
use argusflow_input_contracts::{InputEvent, InputKind, InputOrigin};

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Journal) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recording");
    let session = Session {
        id: "context".into(),
        format: 2,
        created_ms: 0,
        qpc_origin: 0,
        qpc_frequency: 1000,
        capture_session: None,
        policy: "test".into(),
    };
    let journal = Journal::create(&path, &session).unwrap();
    (temp, path, journal)
}

#[test]
fn reads_late_evidence_without_neighboring_targets() {
    let (_temp, path, mut journal) = fixture();
    let input = InputEvent {
        sequence: 1,
        qpc: 100,
        system_time: 0,
        point: Default::default(),
        window: Default::default(),
        foreground_handle: 0,
        origin: InputOrigin::System,
        flags: 0,
        kind: InputKind::Context,
    };
    journal.append(RecordData::Raw(input), 100).unwrap();
    journal.sync().unwrap();
    let raw_context = read(&path, 1).unwrap();
    assert!(matches!(raw_context.action.data, RecordData::Raw(_)));
    assert_eq!(raw_context.records.len(), 1);
    let interaction = Interaction {
        raw: vec![1],
        kind: InteractionKind::SystemKey,
        from_qpc: 100,
        through_qpc: 120,
        window: Default::default(),
        basis: "test".into(),
        related: None,
    };
    journal
        .append(RecordData::Interaction(interaction.clone()), 120)
        .unwrap();
    for index in 0..1100 {
        journal
            .append(
                RecordData::Attempt {
                    raw: vec![999],
                    stage: Stage::Uia,
                    outcome: Outcome::Unavailable,
                    reason: "neighbor".into(),
                },
                200 + index,
            )
            .unwrap();
    }
    journal
        .append(
            RecordData::Attempt {
                raw: vec![1],
                stage: Stage::Uia,
                outcome: Outcome::Unavailable,
                reason: "selected".into(),
            },
            1500,
        )
        .unwrap();
    journal
        .append(
            RecordData::Interaction(Interaction {
                raw: vec![999],
                from_qpc: 200,
                through_qpc: 210,
                ..interaction
            }),
            1501,
        )
        .unwrap();
    journal.sync().unwrap();
    let context = read(&path, 1).unwrap();
    assert_eq!(context.action.id, 2);
    assert_eq!(context.records.len(), 3);
    assert!(context.records.iter().any(
        |record| matches!(&record.data, RecordData::Attempt { reason, .. } if reason == "selected")
    ));
    assert!(context.records.iter().any(
        |record| matches!(&record.data, RecordData::Interaction(value) if value.from_qpc == 200)
    ));
    assert!(read(&path, 42).is_err());
}
