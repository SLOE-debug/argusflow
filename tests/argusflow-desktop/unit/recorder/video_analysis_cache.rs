use super::super::model::Analysis;
use super::*;
#[test]
fn analysis_survives_reopen_and_is_invalidated_by_source_changes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let video = root.join("screen.mp4");
    for name in ["screen.mp4", "session.json", "frames.jsonl"] {
        std::fs::write(root.join(name), b"source").unwrap();
    }
    let decision = Decision {
        sequence: 3,
        analysis: Analysis {
            status: "settled".into(),
            compared_frames: 4,
            regions: vec![[1, 2, 3, 4]],
        },
    };
    Cache::open(root, &video, 100, 200, 100)
        .unwrap()
        .put(root, decision)
        .unwrap();
    assert_eq!(
        Cache::open(root, &video, 100, 200, 100)
            .unwrap()
            .get()
            .unwrap()
            .sequence,
        3
    );
    assert!(
        Cache::open(root, &video, 100, 201, 100)
            .unwrap()
            .get()
            .is_none()
    );
    std::fs::write(root.join("frames.jsonl"), b"more source frames").unwrap();
    assert!(
        Cache::open(root, &video, 100, 200, 100)
            .unwrap()
            .get()
            .is_none()
    );
}
