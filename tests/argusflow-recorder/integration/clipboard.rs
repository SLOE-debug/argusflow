use super::fixtures::*;
use argusflow_input_contracts::*;
use argusflow_recorder::*;
#[test]
fn clipboard_round_trip_requires_committed_raw_and_keeps_sequence_change() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("clipboard");
    let mut writer = RecordingWriter::create(&path, &session(), 500, 4).unwrap();
    let raw = writer.raw(key(67, false, 10), 11).unwrap();
    let evidence = |id| {
        RecordData::Clipboard(Clipboard {
            raw: vec![id],
            from_qpc: 12,
            through_qpc: 15,
            observation: ClipboardObservation {
                previous_sequence: Some(8),
                sequence: 9,
                content: ClipboardContent::Text {
                    text: "同文".into(),
                    truncated: false,
                },
            },
        })
    };
    assert!(writer.derived(evidence(raw.id + 100), 16).is_err());
    writer.derived(evidence(raw.id), 16).unwrap();
    writer.sync(17).unwrap();
    let page = read_page(&path, Cursor::default(), 128).unwrap();
    let item = page
        .records
        .iter()
        .find_map(|r| {
            if let RecordData::Clipboard(c) = &r.data {
                Some(c)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(item.raw, vec![raw.id]);
    assert_eq!(item.observation.previous_sequence, Some(8));
    assert_eq!(item.observation.sequence, 9);
}
