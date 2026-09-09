//! 服务级稳定、版本、缺口和内容复用验收，不依赖真实桌面。
mod faults;
#[path = "../support/memory.rs"]
mod memory;
use argusflow_capture::*;
use argusflow_capture_contracts::*;
use argusflow_core::{Operation, OperationOptions};
use memory::*;
use std::{sync::atomic::Ordering, time::Duration};
fn options() -> OperationOptions {
    OperationOptions::new(Duration::from_millis(500)).unwrap()
}
async fn ready(service: &CaptureService) {
    for _ in 0..50 {
        if !service.sources().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("source not published")
}

#[tokio::test]
async fn accepted_observation_pins_anchor_past_ordinary_expiry() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            anchor_lifetime: Duration::from_millis(50),
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let future = service.observe(
        &anchor,
        PixelRect::new(0, 0, 16, 16).unwrap(),
        ObservationOptions::default(),
        options(),
    );
    tokio::pin!(future);
    tokio::select! { _=&mut future => panic!("must wait"), _=tokio::time::sleep(Duration::from_millis(5))=>{} }
    backend.watermark(500);
    assert_eq!(
        future.await.unwrap().status,
        ObservationStatus::StableUnchanged
    );
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn out_of_order_revision_fails_closed() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    backend.changed(2, 100, vec![1; 1024], PixelRect::new(0, 0, 1, 1).unwrap());
    backend.watermark(500);
    while backend.pending() {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let result = service
        .sample(
            SampleRequest {
                source: SOURCE,
                region: PixelRect::new(0, 0, 4, 4).unwrap(),
                quiet: Duration::from_millis(150),
                previous: None,
            },
            Operation::new(options()),
        )
        .await;
    assert_eq!(
        result.unwrap_err().kind(),
        argusflow_core::FailureKind::Protocol
    );
    assert_eq!(backend.reads.load(Ordering::Acquire), 0);
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn continuing_animation_times_out_with_process_metadata() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let producer = backend.clone();
    let updates = tokio::spawn(async move {
        for revision in 1..10 {
            producer.changed(
                revision,
                20 + revision * 5,
                vec![revision as u8; 1024],
                PixelRect::new(0, 0, 16, 16).unwrap(),
            );
            producer.watermark(20 + revision * 5);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let policy = ObservationOptions {
        minimum: Duration::ZERO,
        quiet: Duration::from_millis(20),
        maximum: Duration::from_millis(30),
        ..Default::default()
    };
    let result = service
        .observe(
            &anchor,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            policy,
            options(),
        )
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::TimedOutUnstable);
    assert!(result.process.changes > 0);
    assert!(result.images.is_empty());
    updates.await.unwrap();
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn returning_to_baseline_keeps_process_but_reads_no_pixels() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let mut changed = vec![0; 1024];
    changed[0] = 255;
    backend.changed(1, 100, changed, PixelRect::new(0, 0, 1, 1).unwrap());
    backend.changed(2, 200, vec![0; 1024], PixelRect::new(0, 0, 1, 1).unwrap());
    backend.watermark(500);
    let result = service
        .observe(
            &anchor,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            ObservationOptions::default(),
            options(),
        )
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::StableUnchanged);
    assert_eq!(result.process.changes, 2);
    assert!(result.images.is_empty());
    assert_eq!(backend.reads.load(Ordering::Relaxed), 0);
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn multiple_anchors_share_visual_version_and_padding() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(30);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let a = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let b = service.pin_at(SOURCE, time(20), options()).await.unwrap();
    let mut changed = vec![0; 1024];
    changed[(8 * 16 + 8) * 4] = 1;
    backend.changed(1, 100, changed, PixelRect::new(8, 8, 1, 1).unwrap());
    backend.watermark(500);
    let r1 = service
        .observe(
            &a,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            ObservationOptions::default(),
            options(),
        )
        .await
        .unwrap();
    let r2 = service
        .observe(
            &b,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            ObservationOptions::default(),
            options(),
        )
        .await
        .unwrap();
    assert_eq!(r1.status, ObservationStatus::StableChanged);
    assert_eq!(r1.after, r2.after);
    assert_eq!(r1.exact_regions, [PixelRect::new(8, 8, 1, 1).unwrap()]);
    assert_eq!(r1.images[0].0, PixelRect::new(0, 0, 16, 16).unwrap());
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn stagnant_health_watermark_cannot_become_stable() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend, CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let policy = ObservationOptions {
        minimum: Duration::ZERO,
        quiet: Duration::from_millis(20),
        maximum: Duration::from_millis(30),
        ..Default::default()
    };
    let result = service
        .observe(
            &anchor,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            policy,
            OperationOptions::new(Duration::from_millis(40)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::TimedOutUnstable);
    assert!(result.after.is_none());
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn merged_presentations_and_eviction_are_explicit() {
    let backend = Memory::new();
    backend.setup();
    backend.emit(BackendEvent::Gap(Gap {
        source: SOURCE,
        from: time(10),
        through: time(40),
        reason: GapReason::Accumulated,
    }));
    backend.watermark(100);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            history_versions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    assert!(matches!(
        service.pin_at(SOURCE, time(20), options()).await,
        Err(CaptureError::HistoryGap(_))
    ));
    for revision in 1..5 {
        backend.changed(
            revision,
            revision * 100,
            vec![revision as u8; 1024],
            PixelRect::new(0, 0, 16, 16).unwrap(),
        );
    }
    backend.watermark(1000);
    while backend.pending() {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(matches!(
        service.pin_at(SOURCE, time(80), options()).await,
        Err(CaptureError::HistoryGap(_))
    ));
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn region_cache_ignores_changes_outside_region() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(500);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let region = PixelRect::new(0, 0, 4, 4).unwrap();
    let first = service
        .sample(
            SampleRequest {
                source: SOURCE,
                region,
                quiet: Duration::from_millis(150),
                previous: None,
            },
            Operation::new(options()),
        )
        .await
        .unwrap();
    assert!(matches!(first.content, SampleContent::Image(_)));
    let reads = backend.reads.load(Ordering::Relaxed);
    let mut bytes = vec![0; 1024];
    bytes[(15 * 16 + 15) * 4] = 255;
    backend.changed(1, 550, bytes, PixelRect::new(15, 15, 1, 1).unwrap());
    backend.watermark(800);
    let second = service
        .sample(
            SampleRequest {
                source: SOURCE,
                region,
                quiet: Duration::from_millis(150),
                previous: Some(first.token),
            },
            Operation::new(options()),
        )
        .await
        .unwrap();
    assert!(matches!(second.content, SampleContent::Unchanged));
    assert_eq!(backend.reads.load(Ordering::Relaxed), reads);
    assert_eq!(second.observed_version.revision, 1);
    service.shutdown(options()).await.unwrap();
}
#[tokio::test]
async fn ignored_region_never_reads_filtered_pixels() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let mut bytes = vec![0; 1024];
    bytes[0] = 1;
    backend.changed(1, 100, bytes, PixelRect::new(0, 0, 1, 1).unwrap());
    backend.watermark(500);
    let policy = ObservationOptions {
        ignored: vec![PixelRect::new(0, 0, 4, 4).unwrap()],
        ..Default::default()
    };
    let result = service
        .observe(
            &anchor,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            policy,
            options(),
        )
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::StableUnchanged);
    assert_eq!(backend.reads.load(Ordering::Relaxed), 0);
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn a_new_snapshot_beyond_watermark_cannot_be_stable() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(400);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let mut changed = vec![0; 1024];
    changed[0] = 1;
    backend.changed(1, 450, changed, PixelRect::new(0, 0, 1, 1).unwrap());
    while backend.pending() {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let result = service
        .observe(
            &anchor,
            PixelRect::new(0, 0, 16, 16).unwrap(),
            ObservationOptions::default(),
            OperationOptions::new(Duration::from_millis(40)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::TimedOutUnstable);
    assert!(result.images.is_empty());
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn subscriptions_report_lost_sequences_without_forging_records() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            metadata_bytes: 4096,
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    let mut slow = service.subscribe_changes();
    let mut fast = service.subscribe_changes();
    for revision in 1..100 {
        backend.changed(
            revision,
            20 + revision,
            vec![revision as u8; 1024],
            PixelRect::new(0, 0, 16, 16).unwrap(),
        );
    }
    backend.watermark(300);
    while backend.pending() {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let gap = slow.poll(1024).unwrap();
    assert!(gap.records.is_empty());
    assert!(gap.lost_sequences.is_some());
    assert_eq!(gap.gaps[0].reason, GapReason::ConsumerLagged);
    let actual = slow.poll(1024).unwrap();
    assert!(!actual.records.is_empty());
    assert!(
        actual
            .records
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    let other = fast.poll(1024).unwrap();
    assert_eq!(other.lost_sequences, gap.lost_sequences);
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn cancelled_sample_releases_admission_and_revoked_anchor_fails() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            reads: 1,
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let request = SampleRequest {
        source: SOURCE,
        region: PixelRect::new(0, 0, 4, 4).unwrap(),
        quiet: Duration::from_millis(150),
        previous: None,
    };
    let operation = Operation::new(options());
    let future = service.sample(request.clone(), operation.clone());
    tokio::pin!(future);
    tokio::select! {_=&mut future=>panic!("must await health"),_=tokio::time::sleep(Duration::from_millis(5))=>{}}
    operation.cancel();
    assert_eq!(
        future.await.unwrap_err().kind(),
        argusflow_core::FailureKind::Cancelled
    );
    backend.watermark(500);
    assert!(
        service
            .sample(request, Operation::new(options()))
            .await
            .is_ok()
    );
    backend.valid.store(false, Ordering::Release);
    assert!(
        service
            .read_regions(&anchor, &[PixelRect::new(0, 0, 1, 1).unwrap()], options())
            .await
            .is_err()
    );
    service.shutdown(options()).await.unwrap();
}
