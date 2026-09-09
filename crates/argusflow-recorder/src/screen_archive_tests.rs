//! 可控呈现序列经共享差分、并行编码、索引和重建的端到端验收。
use crate::{screen_archive::*, screen_archive_writer::ScreenArchiveWriter};
use argusflow_capture::{CaptureBroker, PixelRect};
use argusflow_core::*;

#[test]
fn encoding_failure_publishes_only_the_valid_prefix() {
    let directory =
        std::env::temp_dir().join(format!("argusflow-prefix-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    // 第二帧目标故意成为目录，确保失败发生在有序提交链中间。
    std::fs::create_dir(directory.join(patch_name(ScreenFrameId(2), 0))).unwrap();
    let writer = ScreenArchiveWriter::start(directory.clone()).unwrap();
    let result = writer.result();
    let mut broker = CaptureBroker::default();
    let bounds = InspectionRect {
        x: 0.0,
        y: 0.0,
        width: 2.0,
        height: 1.0,
    };
    for step in 1..=3 {
        let pixels = EvidenceFrame::new(
            bounds,
            2,
            1,
            EvidencePixelFormat::Rgba8,
            vec![step as u8; 8],
        )
        .unwrap();
        let frame = broker
            .publish(capture::ScreenCaptureUpdate {
                source: CaptureSourceId(1),
                generation: CaptureGeneration(1),
                bounds,
                timing: CaptureTiming {
                    presented_us: step,
                    frozen_us: step,
                },
                reset: step == 1,
                accumulated_frames: 1,
                patches: vec![pixels],
            })
            .unwrap()
            .unwrap();
        let record = ScreenFrame {
            id: ScreenFrameId(step),
            source: frame.source,
            generation: frame.generation,
            revision: frame.revision,
            presented_us: step,
            frozen_us: step,
            bounds,
            previous: None,
            changes: vec![bounds],
            patches: Vec::new(),
        };
        writer
            .submit(
                frame,
                record,
                vec![PixelRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                }],
            )
            .unwrap();
    }
    drop(writer);
    let timeline = result.snapshot();
    assert_eq!(timeline.frames.len(), 1);
    assert_eq!(
        timeline.completeness,
        ScreenCompleteness::Incomplete {
            reason: CaptureFailure::Storage
        }
    );
    assert_eq!(
        crate::screen_archive_reader::reconstruct(&directory, &timeline, ScreenFrameId(1))
            .unwrap()
            .pixels(),
        &[1; 8]
    );
    assert!(!directory.join(patch_name(ScreenFrameId(3), 0)).exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn eight_rendered_characters_survive_disk_and_key_up_boundaries() {
    for interval_ms in [100, 50, 33] {
        let root =
            std::env::temp_dir().join(format!("argusflow-screen-test-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4();
        let directory = root.join(id.to_string()).join("evidence");
        std::fs::create_dir_all(&directory).unwrap();
        let writer = ScreenArchiveWriter::start(directory.clone()).unwrap();
        let result = writer.result();
        let mut broker = CaptureBroker::default();
        let bounds = InspectionRect {
            x: -80.0,
            y: 10.0,
            width: 96.0,
            height: 40.0,
        };
        let mut expected = Vec::new();
        let mut pixels = vec![0u8; 96 * 40 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        let mut events = Vec::new();
        for step in 0..=8u64 {
            if step > 0 {
                // 每次追加独立细笔画，包括连续两个 d；不通过复制同一截图充数。
                for y in 3..18 {
                    pixels[(y * 96 + step as usize * 4) * 4] = b"notepadd"[step as usize - 1];
                }
                let mut down = crate::RawTraceEvent {
                    sequence: step * 2 - 1,
                    timestamp_ms: 0,
                    elapsed_ms: 0,
                    input: crate::RawInput::Key {
                        virtual_key: Some(65),
                        scan_code: Some(30),
                        flags: Some(0),
                        phase: crate::InputPhase::Down,
                        text: None,
                        chord: None,
                    },
                    evidence: None,
                    diagnostics: Vec::new(),
                };
                down.elapsed_ms = step * interval_ms;
                let mut up = down.clone();
                up.sequence += 1;
                up.elapsed_ms += 1;
                if let crate::RawInput::Key { phase, .. } = &mut up.input {
                    *phase = crate::InputPhase::Up;
                }
                events.extend([down, up]);
            }
            let presented_us = if step == 0 {
                0
            } else {
                (step * interval_ms + 2) * 1000
            };
            let native =
                EvidenceFrame::new(bounds, 96, 40, EvidencePixelFormat::Rgba8, pixels.clone())
                    .unwrap();
            expected.push(pixels.clone());
            let frame = broker
                .publish(capture::ScreenCaptureUpdate {
                    source: CaptureSourceId(1),
                    generation: CaptureGeneration(1),
                    bounds,
                    timing: CaptureTiming {
                        presented_us,
                        frozen_us: presented_us + 300,
                    },
                    reset: step == 0,
                    accumulated_frames: 1,
                    patches: vec![native],
                })
                .unwrap()
                .unwrap();
            let regions = if step == 0 {
                vec![PixelRect {
                    x: 0,
                    y: 0,
                    width: 96,
                    height: 40,
                }]
            } else {
                frame.changes.to_vec()
            };
            let record = ScreenFrame {
                id: ScreenFrameId(step + 1),
                source: frame.source,
                generation: frame.generation,
                revision: frame.revision,
                presented_us,
                frozen_us: presented_us + 300,
                bounds,
                previous: (step > 0).then_some(ScreenFrameId(step)),
                changes: regions
                    .iter()
                    .map(|r| InspectionRect {
                        x: bounds.x + f64::from(r.x),
                        y: bounds.y + f64::from(r.y),
                        width: r.width.into(),
                        height: r.height.into(),
                    })
                    .collect(),
                patches: Vec::new(),
            };
            writer.submit(frame, record, regions).unwrap();
        }
        drop(writer);
        let mut trace = crate::RecordingTrace {
            schema_version: 3,
            recording_id: id,
            started_at_unix_ms: 0,
            dropped_events: 0,
            timeline: crate::EventTimeline { events },
            screen: result.snapshot(),
        };
        crate::screen_association::associate(&mut trace);
        assert_eq!(trace.screen.frames.len(), 9);
        assert_eq!(trace.screen.completeness, ScreenCompleteness::Complete);
        for (index, event) in trace.timeline.events.iter().step_by(2).enumerate() {
            assert_eq!(
                event.evidence.as_ref().unwrap().screen.after,
                vec![ScreenFrameId(index as u64 + 2)]
            );
        }
        crate::storage::save(&root, &trace).await.unwrap();
        let loaded = crate::history::load(&root, id).await.unwrap();
        for index in (0..9).rev() {
            let frame = crate::screen_archive_reader::reconstruct(
                &directory,
                &loaded.trace.screen,
                ScreenFrameId(index as u64 + 1),
            )
            .unwrap();
            assert_eq!(frame.pixels(), expected[index]);
        }
        // 编辑共享基准后，后继仍是编辑前的真实画面，旧原始区域必须删除。
        let old = directory.join(patch_name(ScreenFrameId(2), 0));
        let edited = crate::privacy_edit::apply(
            &root,
            id,
            vec![crate::PrivacyEdit::Mosaic {
                sequence: 2,
                kind: crate::ScreenshotKind::Screen,
                rect: crate::PrivacyRect {
                    x: 0,
                    y: 0,
                    width: 96,
                    height: 40,
                },
            }],
        )
        .await
        .unwrap();
        assert!(!old.exists());
        assert_ne!(
            crate::screen_archive_reader::reconstruct(
                &directory,
                &edited.trace.screen,
                ScreenFrameId(2)
            )
            .unwrap()
            .pixels(),
            expected[1]
        );
        assert_eq!(
            crate::screen_archive_reader::reconstruct(
                &directory,
                &edited.trace.screen,
                ScreenFrameId(9)
            )
            .unwrap()
            .pixels(),
            expected[8]
        );
        // 路径是本测试显式生成的 UUID 子树，不依赖用户路径。
        std::fs::remove_dir_all(&root).unwrap();
    }
}
