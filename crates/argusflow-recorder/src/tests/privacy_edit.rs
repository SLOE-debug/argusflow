//! 验证隐私处理确实改变磁盘像素、同步局部图并清理被删除记录的文件。
use super::fixtures::{raw, text};
use crate::*;
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionRect, ScreenPoint};

async fn fixture() -> (std::path::PathBuf, uuid::Uuid) {
    let id = uuid::Uuid::new_v4();
    let root = std::env::temp_dir().join(format!("argusflow-privacy-test-{id}"));
    let directory = root.join(id.to_string());
    tokio::fs::create_dir_all(directory.join("evidence"))
        .await
        .unwrap();
    let frame = EvidenceFrame::new(
        InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 6.0,
        },
        8,
        6,
        EvidencePixelFormat::Rgba8,
        vec![255; 8 * 6 * 4],
    )
    .unwrap();
    let store = crate::screenshots::ScreenshotStore::new(directory);
    let mut event = raw(1, text("private content"));
    let evidence = event.evidence.as_mut().unwrap();
    evidence.screenshot = Some(
        store
            .save(1, &frame, 0, 0, Some(ScreenPoint { x: 2, y: 2 }), true)
            .unwrap(),
    );
    evidence.click_target = Some(
        store
            .save_target(1, &frame, 0, 0, Some(ScreenPoint { x: 2, y: 2 }))
            .unwrap(),
    );
    crate::storage::save(
        &root,
        &RecordingTrace {
            screen: crate::ScreenTimeline::default(),
            schema_version: 3,
            recording_id: id,
            started_at_unix_ms: 0,
            timeline: EventTimeline {
                events: vec![event],
            },
            dropped_events: 0,
        },
    )
    .await
    .unwrap();
    (root, id)
}

fn pixels(bytes: Vec<u8>) -> Vec<u8> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut pixels).unwrap();
    pixels
}

#[tokio::test]
async fn mosaic_changes_full_image_and_crop_then_erase_removes_all_variants() {
    let (root, id) = fixture().await;
    let rect = PrivacyRect {
        x: 1,
        y: 1,
        width: 2,
        height: 2,
    };
    crate::privacy_edit::apply(
        &root,
        id,
        vec![PrivacyEdit::Mosaic {
            sequence: 1,
            kind: ScreenshotKind::Window,
            rect,
        }],
    )
    .await
    .unwrap();
    for kind in [ScreenshotKind::Window, ScreenshotKind::Crop] {
        let image = pixels(
            crate::evidence_reader::read(&root, id, 1, kind)
                .await
                .unwrap(),
        );
        assert_eq!(&image[0..4], &[255; 4]);
        assert_eq!(&image[36..40], &[100, 100, 100, 255]);
    }
    let target = pixels(
        crate::evidence_reader::read(&root, id, 1, ScreenshotKind::Target)
            .await
            .unwrap(),
    );
    assert!(target.iter().all(|byte| *byte == 255));
    let updated =
        crate::privacy_edit::apply(&root, id, vec![PrivacyEdit::EraseEvent { sequence: 1 }])
            .await
            .unwrap();
    assert!(updated.trace.timeline.events.is_empty());
    assert!(
        crate::history::load(&root, id)
            .await
            .unwrap()
            .trace
            .timeline
            .events
            .is_empty()
    );
    assert!(
        tokio::fs::read_dir(root.join(id.to_string()).join("evidence"))
            .await
            .unwrap()
            .next_entry()
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        crate::history::list(&root).await.unwrap()[0].screenshot_count,
        0
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn invalid_batch_is_rejected_before_any_original_is_modified() {
    let (root, id) = fixture().await;
    let original = crate::evidence_reader::read(&root, id, 1, ScreenshotKind::Window)
        .await
        .unwrap();
    let result = crate::privacy_edit::apply(
        &root,
        id,
        vec![
            PrivacyEdit::EraseEvent { sequence: 1 },
            PrivacyEdit::Mosaic {
                sequence: 1,
                kind: ScreenshotKind::Window,
                rect: PrivacyRect {
                    x: u32::MAX,
                    y: 0,
                    width: 2,
                    height: 1,
                },
            },
        ],
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        crate::evidence_reader::read(&root, id, 1, ScreenshotKind::Window)
            .await
            .unwrap(),
        original
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}
