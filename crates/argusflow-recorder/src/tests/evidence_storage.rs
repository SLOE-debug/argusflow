use super::fixtures::{raw, text};
use crate::*;
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionRect, ScreenPoint};

#[tokio::test]
async fn timeline_and_png_roundtrip_without_compiled_operations() {
    let id = uuid::Uuid::new_v4();
    let root = std::env::temp_dir().join(format!("argusflow-recorder-test-{id}"));
    let directory = root.join(id.to_string());
    tokio::fs::create_dir_all(directory.join("evidence"))
        .await
        .unwrap();
    let frame = EvidenceFrame::new(
        InspectionRect {
            x: -2.0,
            y: 0.0,
            width: 4.0,
            height: 3.0,
        },
        4,
        3,
        EvidencePixelFormat::Rgba8,
        vec![255; 4 * 3 * 4],
    )
    .unwrap();
    let screenshot = crate::screenshots::ScreenshotStore::new(directory.clone())
        .save(1, &frame, 12, 2, Some(ScreenPoint { x: -1, y: 1 }), true)
        .unwrap();
    // 在 Trace 尚未发布前，图像已经落盘。
    assert!(directory.join(&screenshot.path).exists());
    let mut event = raw(1, text("fixture"));
    event.evidence.as_mut().unwrap().screenshot = Some(screenshot);
    let trace = RecordingTrace {
        schema_version: 2,
        recording_id: id,
        started_at_unix_ms: 123,
        timeline: EventTimeline {
            events: vec![event],
        },
        dropped_events: 0,
    };
    let files = crate::storage::save(&root, &trace).await.unwrap();
    crate::storage::save(&root, &trace).await.unwrap();
    let loaded = crate::history::load(&root, id).await.unwrap();
    assert_eq!(loaded.trace.timeline, trace.timeline);
    let json = serde_json::to_string(&loaded.trace).unwrap();
    for forbidden in ["selector", "normalized", "operation", "preferred_selector"] {
        assert!(!json.contains(forbidden));
    }
    assert_eq!(
        crate::history::list(&root).await.unwrap()[0].screenshot_count,
        1
    );
    for kind in [ScreenshotKind::Window, ScreenshotKind::Crop] {
        let png = crate::evidence_reader::read(&root, id, 1, kind)
            .await
            .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        let info = decoder.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (4, 3));
        assert_eq!(&pixels[..info.buffer_size()], frame.pixels());
    }
    assert!(
        crate::evidence_reader::read(&root, id, 2, ScreenshotKind::Window)
            .await
            .is_err()
    );
    // 局部图像写入失败不得抹掉已经成功保存的完整窗口证据。
    let crop_blocker = directory.join("evidence/2-crop.png");
    tokio::fs::create_dir(&crop_blocker).await.unwrap();
    let full_only = crate::screenshots::ScreenshotStore::new(directory.clone())
        .save(2, &frame, 20, 1, Some(ScreenPoint { x: -1, y: 1 }), true)
        .unwrap();
    assert!(full_only.crop.is_none());
    assert!(full_only.crop_failure.is_some());
    assert!(directory.join(&full_only.path).exists());
    tokio::fs::remove_dir(crop_blocker).await.unwrap();
    tokio::fs::remove_file(directory.join("evidence/2-crop.pending"))
        .await
        .unwrap();
    tokio::fs::remove_file(directory.join("evidence/2.png"))
        .await
        .unwrap();
    for path in [
        files.timeline,
        files.manifest,
        directory.join("evidence/1.png"),
        directory.join("evidence/1-crop.png"),
    ] {
        tokio::fs::remove_file(path).await.unwrap();
    }
    tokio::fs::remove_dir(directory.join("evidence"))
        .await
        .unwrap();
    tokio::fs::remove_dir(directory).await.unwrap();
    tokio::fs::remove_dir(root).await.unwrap();
}
