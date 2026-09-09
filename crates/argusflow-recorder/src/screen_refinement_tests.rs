//! 真正执行 GPU 后处理、去重和落盘后的逐像素重建。
use super::*;
use argusflow_core::*;

#[test]
#[ignore = "requires hardware GPU compute; uses synthetic images and no desktop capture"]
fn gpu_refinement_retains_eight_changes_and_removes_repaints() {
    let directory = std::env::temp_dir().join(format!("argusflow-refine-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let source = argusflow_windows::capture::WindowsEventCapture::default();
    let mut differ = capture::ScreenCaptureSource::create_pixel_differ(&source).unwrap();
    let bounds = InspectionRect {
        x: -80.0,
        y: -20.0,
        width: 96.0,
        height: 40.0,
    };
    let mut broker = argusflow_capture::CaptureBroker::default();
    let writer =
        crate::screen_archive_writer::ScreenArchiveWriter::start(directory.clone()).unwrap();
    let result = writer.result();
    let mut pixels = vec![0; 96 * 40 * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let mut expected = Vec::new();
    for step in 0..=16u64 {
        if step % 2 == 1 {
            let character = (step / 2) as usize;
            for y in 3..18 {
                pixels[(y * 96 + 4 + character * 4) * 4] = b"notepadd"[character];
            }
        }
        expected.push(pixels.clone());
        let update = capture::ScreenCaptureUpdate {
            source: CaptureSourceId(1),
            generation: CaptureGeneration(1),
            bounds,
            timing: CaptureTiming {
                presented_us: step * 25_000,
                frozen_us: step * 25_000 + 200,
            },
            reset: step == 0,
            accumulated_frames: 1,
            patches: vec![
                EvidenceFrame::new(bounds, 96, 40, EvidencePixelFormat::Rgba8, pixels.clone())
                    .unwrap(),
            ],
        };
        let frame = broker.publish_candidates(update).unwrap().unwrap();
        let record = ScreenFrame {
            id: ScreenFrameId(step + 1),
            source: frame.source,
            generation: frame.generation,
            revision: frame.revision,
            presented_us: frame.timing.presented_us,
            frozen_us: frame.timing.frozen_us,
            bounds,
            previous: (step > 0).then_some(ScreenFrameId(step)),
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
                    width: 96,
                    height: 40,
                }],
            )
            .unwrap();
    }
    drop(writer);
    let mut raw = result.snapshot();
    raw.duration_us = 5_000_000;
    assert_eq!(
        raw.frames.len(),
        17,
        "recording must retain even identical native repaints"
    );
    let refined = refine(&directory, &raw, differ.as_mut()).unwrap();
    assert_eq!(refined.timeline.frames.len(), 9);
    assert_eq!(refined.timeline.duration_us, 5_000_000);
    for frame in &refined.timeline.frames {
        let pixels =
            crate::screen_archive_reader::reconstruct(&directory, &refined.timeline, frame.id)
                .unwrap();
        assert_eq!(pixels.pixels(), expected[frame.id.0 as usize - 1]);
        if frame.id.0 > 1 {
            assert_eq!(frame.changes.len(), 1);
            assert_eq!(frame.changes[0].width, 1.0);
        }
    }
    // 未提交结果 Drop 仅清理新区域，原候选归档依然逐像素可读。
    drop(refined);
    assert_eq!(
        crate::screen_archive_reader::reconstruct(&directory, &raw, ScreenFrameId(17))
            .unwrap()
            .pixels(),
        expected[16]
    );
    struct Failed;
    impl capture::refinement::PixelDiffer for Failed {
        fn compare(
            &mut self,
            _: &EvidenceFrame,
            _: &EvidenceFrame,
        ) -> Result<Vec<InspectionRect>, CaptureError> {
            Err(CaptureError::CaptureUnavailable {
                message: "test device removed".into(),
            })
        }
    }
    assert!(refine(&directory, &raw, &mut Failed).is_err());
    let (precise, obsolete) = refine(&directory, &raw, differ.as_mut()).unwrap().commit();
    for path in obsolete {
        std::fs::remove_file(path).unwrap();
    }
    assert_eq!(
        crate::screen_archive_reader::reconstruct(&directory, &precise, ScreenFrameId(16))
            .unwrap()
            .pixels(),
        expected[15]
    );
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 9);
    std::fs::remove_dir_all(&directory).unwrap();
}
