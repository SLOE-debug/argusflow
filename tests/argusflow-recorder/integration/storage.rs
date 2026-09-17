use super::fixtures::*;
use argusflow_input_contracts::*;
use argusflow_recorder::*;
use std::io::Write;
#[test]
fn optional_sources_remain_explicit_without_counting_as_failed_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut journal = Journal::create(&path, &session()).unwrap();
    journal
        .append(
            RecordData::Attempt {
                raw: vec![],
                stage: Stage::Ocr,
                outcome: Outcome::NotConfigured,
                reason: "未启用 OCR".into(),
            },
            1,
        )
        .unwrap();
    journal.sync().unwrap();
    let page = read_page(&path, Cursor::default(), 128).unwrap();
    let RecordData::Attempt { outcome, .. } = page.records[0].data else {
        panic!("attempt")
    };
    assert_eq!(outcome, Outcome::NotConfigured);
    assert!(!outcome.is_evidence_gap());
    assert!(!Outcome::Complete.is_evidence_gap());
    for outcome in [
        Outcome::Unavailable,
        Outcome::Unresolved,
        Outcome::TimedOut,
        Outcome::BudgetExceeded,
        Outcome::Stale,
        Outcome::Cancelled,
        Outcome::Failed,
    ] {
        assert!(outcome.is_evidence_gap());
    }
}
#[test]
fn writer_confirms_raw_before_derivation_and_tracks_sync() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut w = RecordingWriter::create(&path, &session(), 500, 4).unwrap();
    let raw = w.raw(button(true, 10), 11).unwrap();
    assert_eq!(w.watermarks(), (1, 0));
    assert!(w.normalize(&raw, 12).unwrap().is_none());
    let result = w.derived(
        RecordData::Attempt {
            raw: vec![99],
            stage: Stage::Visual,
            outcome: Outcome::Failed,
            reason: "bad".into(),
        },
        13,
    );
    assert!(result.is_err());
    let (_, synced) = w.sync(20).unwrap();
    assert_eq!(w.watermarks(), (synced, synced));
}
#[test]
fn partial_tail_is_quarantined_and_reopen_reports_exact_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut w = Journal::create(&path, &session()).unwrap();
    w.append(RecordData::Raw(button(true, 10)), 11).unwrap();
    w.sync().unwrap();
    drop(w);
    let valid = std::fs::metadata(path.join("events.afr")).unwrap().len();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path.join("events.afr"))
        .unwrap();
    file.write_all(b"AFR1\xff").unwrap();
    drop(file);
    let page = read_page(&path, Cursor::default(), 128).unwrap();
    assert_eq!(page.records.len(), 1);
    assert!(page.tail.is_some());
    assert_eq!(page.cursor.offset, valid);
    let report = recover(&path, 100).unwrap();
    assert_eq!(report.cursor.offset, valid);
    assert!(std::fs::metadata(path.join("events.afr")).unwrap().len() > valid);
    assert!(
        read_page(&path, Cursor::default(), 128)
            .unwrap()
            .records
            .iter()
            .any(|r| matches!(
                r.data,
                RecordData::State {
                    phase: SessionPhase::Interrupted,
                    ..
                }
            ))
    );
    assert!(path.join(format!("recovery-tail-{valid}.bin")).exists());
}
#[test]
fn corrupt_payload_is_never_reinterpreted_as_complete() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut w = Journal::create(&path, &session()).unwrap();
    w.append(RecordData::Raw(button(true, 10)), 11).unwrap();
    drop(w);
    let mut bytes = std::fs::read(path.join("events.afr")).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(path.join("events.afr"), bytes).unwrap();
    let page = read_page(&path, Cursor::default(), 128).unwrap();
    assert!(page.records.is_empty());
    assert!(page.tail.unwrap().contains("校验"));
}
#[test]
fn attachment_publication_deduplicates_and_reopens() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut w = Journal::create(&path, &session()).unwrap();
    let a = publish_image(&path, b"fixture encoded image", [1, 1]).unwrap();
    let b = publish_image(&path, b"fixture encoded image", [1, 1]).unwrap();
    assert_eq!(a.path, b.path);
    let visual = Visual {
        raw: vec![1],
        relation: Relation::Before,
        version: PixelVersion {
            session: "1".into(),
            source: 1,
            generation: 1,
            revision: 1,
        },
        presented_ns: None,
        acquired_ns: 0,
        frozen_ns: 0,
        from_ns: 0,
        through_ns: 0,
        region: [0, 0, 1, 1],
        screen_origin: [-1, -1],
        dpi: [144, 144],
        status: VisualStatus::Anchored,
        decision: EvidenceDecision::VisualRequired("fixture".into()),
        image: Some(a.clone()),
    };
    w.append(RecordData::Raw(button(true, 10)), 11).unwrap();
    w.append(RecordData::Visual(visual), 12).unwrap();
    w.sync().unwrap();
    drop(w);
    let destination = path;
    assert_eq!(load_session(&destination).unwrap().id, "test-session");
    assert_eq!(
        read_attachment(&destination, &a).unwrap(),
        b"fixture encoded image"
    );
    assert_eq!(
        read_page(&destination, Cursor::default(), 128)
            .unwrap()
            .records
            .len(),
        2
    );
    let mut traversal = a;
    traversal.path = "../secret.png".into();
    assert!(read_attachment(&destination, &traversal).is_err());
}
#[test]
fn pause_resume_stop_and_own_injected_facts_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session");
    let mut w = RecordingWriter::create(&path, &session(), 500, 4).unwrap();
    let mut event = button(true, 10);
    event.origin = InputOrigin::ArgusFlow;
    let raw = w.raw(event, 11).unwrap();
    assert!(w.normalize(&raw, 12).unwrap().is_none());
    w.transition(SessionPhase::Paused, "pause".into(), 20)
        .unwrap();
    assert!(w.raw(button(false, 30), 31).is_err());
    w.transition(SessionPhase::Recording, "resume".into(), 40)
        .unwrap();
    let raw = w.raw(button(false, 50), 51).unwrap();
    let action = w.normalize(&raw, 52).unwrap().unwrap();
    assert!(matches!(
        action.data,
        RecordData::Interaction(Interaction {
            kind: InteractionKind::Unresolved,
            ..
        })
    ));
    w.transition(SessionPhase::Stopping, "stop".into(), 60)
        .unwrap();
    w.transition(SessionPhase::Stopped, "done".into(), 70)
        .unwrap();
    assert!(
        w.transition(SessionPhase::Recording, "bad".into(), 80)
            .is_err()
    );
    assert!(Journal::create(&path, &session()).is_err());
}
#[test]
fn failed_attachment_does_not_return_a_reference() {
    let temp = tempfile::tempdir().unwrap();
    assert!(publish_image(temp.path(), b"x", [1, 1]).is_err());
}

#[test]
fn oversized_evidence_becomes_explicit_gap_without_losing_raw() {
    let command = RecorderCommand::evidence(RecordData::Attempt {
        raw: vec![1],
        stage: Stage::Ocr,
        outcome: Outcome::Complete,
        reason: "x".repeat(MAX_RECORD_BYTES),
    });
    assert!(matches!(
        command,
        RecorderCommand::Append(RecordData::Attempt {
            outcome: Outcome::BudgetExceeded,
            ..
        })
    ));
}

#[test]
fn stopping_closes_all_pending_raw_records() {
    let temp = tempfile::tempdir().unwrap();
    let mut writer =
        RecordingWriter::create(&temp.path().join("session"), &session(), 500, 4).unwrap();
    let first = writer.raw(button(true, 10), 11).unwrap();
    let last = writer.raw(button(false, 20), 21).unwrap();
    let cancelled = writer.finish_pending("stop", 22).unwrap();
    assert_eq!(cancelled.len(), 1);
    assert!(
        matches!(&cancelled[0].data, RecordData::Attempt { raw, outcome: Outcome::Cancelled, .. } if *raw == vec![first.id, last.id])
    );
    assert!(writer.finish_pending("again", 23).unwrap().is_empty());
}
