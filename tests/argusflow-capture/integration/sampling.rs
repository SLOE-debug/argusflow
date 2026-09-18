//! 完整帧的精确区域采样、健康水位、取消及来源失效回归。
use argusflow_capture::FrameSampler;
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::{sync::Arc, time::Duration};
#[path = "../support/frames.rs"]
mod frames;
use frames::{Frames, frame};
fn request(previous: Option<ContentToken>, quiet: u64) -> SampleRequest {
    SampleRequest {
        source: SourceId(1),
        region: PixelRect::new(0, 0, 4, 4).unwrap(),
        quiet: Duration::from_millis(quiet),
        previous,
    }
}
fn operation() -> Operation {
    Operation::new(OperationOptions::new(Duration::from_millis(40)).unwrap())
}

#[tokio::test]
async fn completed_readback_after_watermark_can_prove_stability() {
    let source = Frames::new();
    source
        .history
        .lock()
        .unwrap()
        .frames
        .push(frame(2, 1005, None));
    let result = FrameSampler::new(source)
        .unwrap()
        .sample(request(None, 150), operation())
        .await
        .unwrap();
    assert_eq!(result.observed_version.revision, 2);
    assert_eq!(result.observed_through, ClockTime(1_000_000_000));
}

#[tokio::test]
async fn completed_readback_after_watermark_still_checks_pixels() {
    let source = Frames::new();
    source
        .history
        .lock()
        .unwrap()
        .frames
        .push(frame(2, 1005, Some((1, 1))));
    let result = FrameSampler::new(source)
        .unwrap()
        .sample(request(None, 150), operation())
        .await;
    assert!(matches!(result, Err(error) if error.kind() == FailureKind::Timeout));
}

#[tokio::test]
async fn unchanged_content_and_zero_quiet_confirmation() {
    let source = Frames::new();
    let sampler = FrameSampler::new(source.clone()).unwrap();
    let first = sampler
        .sample(request(None, 150), operation())
        .await
        .unwrap();
    assert!(matches!(first.content, SampleContent::Image(_)));
    assert_eq!(first.token.snapshot.bounds.x(), -100);
    source
        .history
        .lock()
        .unwrap()
        .frames
        .push(frame(2, 900, None));
    let shared_image = first.token.image.clone();
    let second = sampler
        .sample(request(Some(first.token), 0), operation())
        .await
        .unwrap();
    assert!(matches!(second.content, SampleContent::Unchanged));
    assert_eq!(second.observed_version.revision, 2);
    assert!(std::ptr::eq(
        shared_image.bytes(),
        second.token.image.bytes()
    ));
    assert_eq!(second.token.snapshot.version.revision, 2);
    assert_eq!(
        source.refreshes.load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}
#[tokio::test]
async fn one_pixel_change_invalidates_confirmation() {
    let source = Frames::new();
    let sampler = FrameSampler::new(source.clone()).unwrap();
    let first = sampler
        .sample(request(None, 150), operation())
        .await
        .unwrap();
    source
        .history
        .lock()
        .unwrap()
        .frames
        .push(frame(2, 900, Some((1, 1))));
    let second = sampler
        .sample(request(Some(first.token), 0), operation())
        .await
        .unwrap();
    assert!(matches!(second.content, SampleContent::Image(_)));
}
#[tokio::test]
async fn changes_outside_requested_region_do_not_delay_ocr() {
    let source = Frames::new();
    source
        .history
        .lock()
        .unwrap()
        .frames
        .push(frame(2, 950, Some((7, 1))));
    let sampler = FrameSampler::new(source).unwrap();
    assert!(
        sampler
            .sample(request(None, 150), operation())
            .await
            .is_ok()
    );
}
#[tokio::test]
async fn recent_change_and_stalled_watermark_time_out() {
    for stale in [false, true] {
        let source = Frames::new();
        if stale {
            source.history.lock().unwrap().checked = ClockTime(900_000_000);
        } else {
            source
                .history
                .lock()
                .unwrap()
                .frames
                .push(frame(2, 950, Some((1, 1))));
        }
        let result = FrameSampler::new(source)
            .unwrap()
            .sample(request(None, 150), operation())
            .await;
        assert!(matches!(result,Err(error) if error.kind()==FailureKind::Timeout));
    }
}
#[tokio::test]
async fn transient_change_back_to_baseline_is_not_quiet() {
    let source = Frames::new();
    source
        .history
        .lock()
        .unwrap()
        .frames
        .extend([frame(2, 900, Some((1, 1))), frame(3, 950, None)]);
    let result = FrameSampler::new(source)
        .unwrap()
        .sample(request(None, 150), operation())
        .await;
    assert!(matches!(result,Err(error) if error.kind()==FailureKind::Timeout));
}
#[tokio::test]
async fn rebuild_and_shutdown_revoke_existing_tokens() {
    let source = Frames::new();
    let sampler = FrameSampler::new(source.clone()).unwrap();
    let first = sampler
        .sample(request(None, 150), operation())
        .await
        .unwrap();
    assert!(first.token.snapshot.validity.valid());
    source.history.lock().unwrap().source.generation = 2;
    assert!(!first.token.snapshot.validity.valid());
    source.shutdown(operation()).await.unwrap();
    assert!(sampler.sample(request(None, 0), operation()).await.is_err());
}
#[tokio::test]
async fn invalid_region_downscaled_image_and_cancellation_are_rejected() {
    let source = Frames::new();
    let sampler = FrameSampler::new(source.clone()).unwrap();
    let mut bad = request(None, 0);
    bad.region = PixelRect::new(7, 0, 4, 4).unwrap();
    assert!(sampler.sample(bad, operation()).await.is_err());
    let cancelled = operation();
    cancelled.cancel();
    assert!(
        matches!(sampler.sample(request(None,0),cancelled).await,Err(error) if error.kind()==FailureKind::Cancelled)
    );
    {
        let mut history = source.history.lock().unwrap();
        history.source.bounds = ScreenRect::new(0, 0, 16, 8).unwrap();
        let mut scaled = (*frame(1, 0, None)).clone();
        scaled.source = history.source.clone();
        history.frames = vec![Arc::new(scaled)];
    }
    assert!(
        matches!(sampler.sample(request(None,0),operation()).await,Err(error) if error.kind()==FailureKind::Unsupported)
    );
}
#[tokio::test]
async fn missing_stable_history_times_out() {
    let source = Frames::new();
    source.history.lock().unwrap().frames = vec![frame(2, 950, None)];
    assert!(
        matches!(FrameSampler::new(source).unwrap().sample(request(None,150),operation()).await,Err(error) if error.kind()==FailureKind::Timeout)
    );
}
