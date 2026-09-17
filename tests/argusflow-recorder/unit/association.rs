use super::*;
use crate::*;

#[test]
fn later_inputs_share_observation_interval_and_overflow_is_explicit() {
    let session = Session {
        id: "test".into(),
        format: 2,
        created_ms: 0,
        qpc_origin: 0,
        qpc_frequency: 1_000_000_000,
        capture_session: Some("1".into()),
        policy: "test".into(),
    };
    let mut index = TemporalIndex::new(&session);
    for id in 1..=9000 {
        index.push(id, id as i64);
    }
    let record = Record {
        id: 9001,
        written_qpc: 9001,
        data: RecordData::Visual(Visual {
            raw: vec![1],
            relation: Relation::After,
            version: PixelVersion {
                session: "1".into(),
                source: 1,
                generation: 1,
                revision: 1,
            },
            presented_ns: Some(8900),
            acquired_ns: 8901,
            frozen_ns: 8902,
            from_ns: 1,
            through_ns: 9000,
            region: [0, 0, 1, 1],
            screen_origin: [-1, -1],
            dpi: [144, 144],
            status: VisualStatus::Observed,
            decision: EvidenceDecision::VisualRequired("test".into()),
            image: None,
        }),
    };
    let RecordData::Association { records, relation } = index.associate(&record).unwrap() else {
        panic!("association")
    };
    assert_eq!(records[0], record.id);
    assert_eq!(records.len(), 2048);
    assert!(records[1..].iter().all(|id| *id > 1 && *id <= 9000));
    assert!(relation.contains("截断=true"));
    index.reset();
    assert!(index.inputs.is_empty());
}
