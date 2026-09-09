//! 共享差分必须保留微小变化，同时忽略没有颜色语义的字节。

use super::*;
use argusflow_core::EvidencePixelFormat;

fn view(pixels: &[u8], format: EvidencePixelFormat) -> PixelView<'_> {
    PixelView::new(pixels, 65, 33, 65 * 4, format).unwrap()
}

#[test]
fn single_pixels_and_equal_luminance_colors_are_not_lost() {
    let original = vec![0; 65 * 33 * 4];
    let mut current = original.clone();
    current[0] = 1;
    current[(65 * 33 - 1) * 4 + 2] = 1;
    let changes = compare(
        view(&original, EvidencePixelFormat::Bgrx8),
        view(&current, EvidencePixelFormat::Bgrx8),
        None,
    )
    .unwrap();
    assert_eq!(changes.changed_pixels(), 2);
    assert_eq!(
        changes.regions(),
        &[
            PixelRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1
            },
            PixelRect {
                x: 64,
                y: 32,
                width: 1,
                height: 1
            },
        ]
    );
}

#[test]
fn reserved_channel_and_row_padding_are_ignored() {
    let original = vec![0; 65 * 33 * 4];
    let mut current = original.clone();
    for pixel in current.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    assert_eq!(
        compare(
            view(&original, EvidencePixelFormat::Bgrx8),
            view(&current, EvidencePixelFormat::Bgrx8),
            None
        )
        .unwrap()
        .changed_pixels(),
        0
    );
    assert_eq!(
        compare(
            view(&original, EvidencePixelFormat::Rgba8),
            view(&current, EvidencePixelFormat::Rgba8),
            None
        )
        .unwrap()
        .changed_pixels(),
        65 * 33
    );
    let a = [0; 16];
    let mut b = a;
    b[7] = 123;
    let original = PixelView::new(&a, 1, 2, 8, EvidencePixelFormat::Rgba8).unwrap();
    let current = PixelView::new(&b, 1, 2, 8, EvidencePixelFormat::Rgba8).unwrap();
    assert_eq!(
        compare(original, current, None).unwrap().changed_pixels(),
        0
    );
}

#[test]
fn candidates_deduplicate_tiles_and_validate_bounds() {
    let original = vec![0; 65 * 33 * 4];
    let frame = view(&original, EvidencePixelFormat::Bgrx8);
    let candidate = PixelRect {
        x: 64,
        y: 32,
        width: 1,
        height: 1,
    };
    assert_eq!(
        compare(frame, frame, Some(&[candidate, candidate]))
            .unwrap()
            .compared_pixels(),
        1
    );
    assert_eq!(
        compare(frame, frame, Some(&[])).unwrap().compared_pixels(),
        0
    );
    assert!(
        compare(
            frame,
            frame,
            Some(&[PixelRect {
                width: 2,
                ..candidate
            }])
        )
        .is_err()
    );
}

#[test]
fn snapshots_reconstruct_eight_overlapping_changes_and_share_unchanged_blocks() {
    use argusflow_core::{EvidenceFrame, InspectionRect};
    let bounds = InspectionRect {
        x: -10.0,
        y: 2.0,
        width: 65.0,
        height: 33.0,
    };
    let mut pixels = vec![0; 65 * 33 * 4];
    let baseline =
        EvidenceFrame::new(bounds, 65, 33, EvidencePixelFormat::Bgrx8, pixels.clone()).unwrap();
    let mut snapshot = FrameSnapshot::from_frame(&baseline);
    let original = snapshot.clone();
    for value in 1..=8 {
        pixels[0] = value;
        let frame =
            EvidenceFrame::new(bounds, 65, 33, EvidencePixelFormat::Bgrx8, pixels.clone()).unwrap();
        let (next, changes) = snapshot.update(&frame, None).unwrap();
        assert_eq!(
            changes,
            vec![PixelRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1
            }]
        );
        assert_eq!(next.materialize().unwrap().pixels(), pixels);
        assert_eq!(
            next.blocks()[1].pixels().as_ptr(),
            original.blocks()[1].pixels().as_ptr()
        );
        snapshot = next;
    }
    assert_eq!(original.materialize().unwrap().pixels()[0], 0);
}

#[test]
fn latest_cursors_accumulate_changes_and_ordered_cursor_reports_history_loss() {
    use argusflow_core::*;
    use std::sync::Arc;
    let pixels = EvidenceFrame::new(
        InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 65.0,
            height: 33.0,
        },
        65,
        33,
        EvidencePixelFormat::Bgrx8,
        vec![0; 65 * 33 * 4],
    )
    .unwrap();
    let snapshot = Arc::new(FrameSnapshot::from_frame(&pixels));
    let stream = CaptureStream::new(CaptureSourceId(1), 3, 128 * 1024 * 1024).unwrap();
    let publish = |revision| {
        stream
            .publish(FrameUpdate {
                source: CaptureSourceId(1),
                generation: CaptureGeneration(1),
                revision: CaptureRevision(revision),
                timing: CaptureTiming {
                    presented_us: revision,
                    frozen_us: revision,
                },
                changes: vec![PixelRect {
                    x: revision as u32,
                    y: 0,
                    width: 1,
                    height: 1,
                }]
                .into(),
                snapshot: snapshot.clone(),
            })
            .unwrap()
    };
    let mut latest = stream.subscribe(CaptureDelivery::Latest);
    let mut ordered = stream.subscribe(CaptureDelivery::Ordered);
    publish(1);
    assert!(latest.next().unwrap().unwrap().reset);
    ordered.next().unwrap().unwrap();
    publish(2);
    publish(3);
    let delivery = latest.next().unwrap().unwrap();
    assert_eq!(delivery.frame.revision, CaptureRevision(3));
    assert_eq!(delivery.changes.len(), 2);
    assert!(!delivery.reset);
    publish(4);
    publish(5);
    assert!(matches!(ordered.next(), Err(CaptureFailure::HistoryGap)));
    assert_eq!(
        latest.next().unwrap().unwrap().frame.revision,
        CaptureRevision(5)
    );
}
