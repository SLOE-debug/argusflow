//! 有限资源、来源隔离与拓扑重建的确定性故障注入，不替代多屏硬件验收。
use super::{memory::*, options, ready};
use argusflow_capture::*;
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

async fn consumed(backend: &Memory) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while backend.pending() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}
fn region() -> PixelRect {
    PixelRect::new(0, 0, 4, 4).unwrap()
}
fn request(source: SourceId) -> SampleRequest {
    SampleRequest {
        source,
        region: region(),
        quiet: Duration::from_millis(150),
        previous: None,
    }
}

#[tokio::test]
async fn pinned_anchor_survives_unpinned_history_eviction() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            history_versions: 2,
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    for revision in 1..=5 {
        let mut bytes = vec![0; 1024];
        bytes[0] = revision as u8;
        backend.changed(
            revision,
            revision * 100,
            bytes,
            PixelRect::new(0, 0, 1, 1).unwrap(),
        );
    }
    backend.watermark(1000);
    consumed(&backend).await;
    assert!(matches!(
        service.pin_at(SOURCE, time(10), options()).await,
        Err(CaptureError::HistoryGap(_))
    ));
    let result = service
        .observe(&anchor, region(), ObservationOptions::default(), options())
        .await
        .unwrap();
    assert_eq!(result.status, ObservationStatus::StableChanged);
    assert_eq!(result.before.revision, 0);
    assert_eq!(result.after.unwrap().revision, 5);
    assert_eq!(result.process.changes, 5);
    assert_eq!(result.images[0].1.bytes()[0], 5);
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn cancelled_observation_returns_permit_and_consumer_drop_keeps_service_alive() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(
        backend.clone(),
        CaptureConfig {
            observations: 1,
            ..Default::default()
        },
    )
    .unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let mut pending =
        Box::pin(service.observe(&anchor, region(), ObservationOptions::default(), options()));
    tokio::select! { _=&mut pending=>panic!("must wait for health"),_=tokio::time::sleep(Duration::from_millis(5))=>{} }
    let busy = service
        .observe(&anchor, region(), ObservationOptions::default(), options())
        .await;
    assert!(matches!(busy,Err(error) if error.kind()==FailureKind::Busy));
    drop(pending);
    let survivor = service.clone();
    drop(service);
    backend.watermark(500);
    assert_eq!(
        survivor
            .observe(&anchor, region(), ObservationOptions::default(), options())
            .await
            .unwrap()
            .status,
        ObservationStatus::StableUnchanged
    );
    let mut subscription = survivor.subscribe_changes();
    survivor.shutdown(options()).await.unwrap();
    assert!(matches!(subscription.poll(1),Err(error) if error.kind()==FailureKind::Closed));
}

#[tokio::test]
async fn source_watermarks_generations_and_failure_remain_independent() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(500);
    let other = Memory::new();
    let second = SourceId(8);
    let bounds = ScreenRect::new(-400, 300, 16, 16).unwrap();
    let mut snapshot = (*other.frame(0, 1, vec![7; 1024])).clone();
    snapshot.version.source = second;
    snapshot.version.generation = 7;
    snapshot.bounds = bounds;
    backend.emit(BackendEvent::Source(SourceInfo {
        id: second,
        name: "second".into(),
        generation: 7,
        bounds,
        rotation: Rotation::Identity,
        dpi: (144, 144),
        state: SourceState::Ready,
        failure: None,
    }));
    backend.emit(BackendEvent::Baseline(Arc::new(snapshot)));
    backend.emit(BackendEvent::Watermark {
        source: second,
        generation: 7,
        through: time(100),
    });
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    service
        .sample(request(SOURCE), Operation::new(options()))
        .await
        .unwrap();
    assert_eq!(
        service
            .sample(
                request(second),
                Operation::new(OperationOptions::new(Duration::from_millis(25)).unwrap())
            )
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Timeout
    );
    let mut failed = service
        .sources()
        .into_iter()
        .find(|info| info.id == SOURCE)
        .unwrap();
    failed.state = SourceState::Unavailable;
    backend.valid.store(false, Ordering::Release);
    backend.emit(BackendEvent::Source(failed));
    backend.emit(BackendEvent::Watermark {
        source: second,
        generation: 7,
        through: time(500),
    });
    let result = service
        .sample(request(second), Operation::new(options()))
        .await
        .unwrap();
    assert_eq!(result.observed_version.source, second);
    assert_eq!(result.observed_version.generation, 7);
    assert_eq!(
        result.token.snapshot.bounds.project(region()).unwrap().x(),
        -400
    );
    let SampleContent::Image(image) = result.content else {
        panic!("first sample requires image")
    };
    assert_eq!(image.bytes()[0], 7);
    assert_eq!(
        service
            .sample(request(SOURCE), Operation::new(options()))
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Unavailable
    );
    service.shutdown(options()).await.unwrap();
}

#[tokio::test]
async fn topology_change_revokes_pending_observation_and_samples_new_generation() {
    let backend = Memory::new();
    backend.setup();
    backend.watermark(20);
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    ready(&service).await;
    let anchor = service.pin_at(SOURCE, time(10), options()).await.unwrap();
    let mut pending =
        Box::pin(service.observe(&anchor, region(), ObservationOptions::default(), options()));
    tokio::select! { _=&mut pending=>panic!("must wait for health"),_=tokio::time::sleep(Duration::from_millis(5))=>{} }
    backend.valid.store(false, Ordering::Release);
    let mut info = service.sources()[0].clone();
    info.generation = 2;
    info.bounds = ScreenRect::new(100, 200, 16, 16).unwrap();
    info.dpi = (192, 192);
    backend.emit(BackendEvent::Gap(Gap {
        source: SOURCE,
        from: time(20),
        through: time(100),
        reason: GapReason::TopologyChanged,
    }));
    backend.emit(BackendEvent::Source(info.clone()));
    let rebuilt = Memory::new();
    let mut snapshot = (*rebuilt.frame(0, 100, vec![9; 1024])).clone();
    snapshot.version.generation = 2;
    snapshot.bounds = info.bounds;
    backend.emit(BackendEvent::Baseline(Arc::new(snapshot)));
    backend.now.store(time(500).0, Ordering::Release);
    backend.emit(BackendEvent::Watermark {
        source: SOURCE,
        generation: 2,
        through: time(500),
    });
    let interrupted = pending.await.unwrap();
    assert!(matches!(
        interrupted.status,
        ObservationStatus::HistoryGap | ObservationStatus::SourceUnavailable
    ));
    assert!(interrupted.images.is_empty());
    assert!(
        matches!(service.read_regions(&anchor,&[region()],options()).await,Err(error) if error.kind()==FailureKind::StaleHandle)
    );
    let fresh = service
        .sample(request(SOURCE), Operation::new(options()))
        .await
        .unwrap();
    assert_eq!(fresh.observed_version.generation, 2);
    assert_eq!(fresh.token.snapshot.bounds.x(), 100);
    service.shutdown(options()).await.unwrap();
}
