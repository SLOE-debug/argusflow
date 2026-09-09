//! 原生桌面跟踪与按需区域采样；不保存用户屏幕、不操作浏览器。
use argusflow_capture::{CaptureConfig, CaptureService};
use argusflow_capture_contracts::*;
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::DxgiBackend;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "requires visible SDR Windows desktop and hardware DXGI"]
async fn shared_dxgi_tracks_without_pixel_readback_and_recovers() {
    let backend = Arc::new(DxgiBackend::start(BackendConfig::default()).unwrap());
    let service = CaptureService::start(backend.clone(), CaptureConfig::default()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let source = loop {
        if let Some(source) = service
            .sources()
            .into_iter()
            .find(|source| source.state == SourceState::Ready)
        {
            break source;
        }
        assert!(
            Instant::now() < deadline,
            "sources: {:?}",
            service.sources()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    tokio::time::sleep(Duration::from_millis(250)).await;
    let before = service.stats();
    assert_eq!(
        before.pixel_readback_bytes, 0,
        "tracking alone must never read image pixels"
    );
    let region = PixelRect::new(
        0,
        0,
        32.min(source.bounds.width()),
        32.min(source.bounds.height()),
    )
    .unwrap();
    let request = SampleRequest {
        source: source.id,
        region,
        quiet: Duration::from_millis(150),
        previous: None,
    };
    let sample = service
        .sample(
            request.clone(),
            Operation::new(OperationOptions::new(Duration::from_secs(5)).unwrap()),
        )
        .await
        .unwrap();
    let SampleContent::Image(image) = &sample.content else {
        panic!("first request requires pixels")
    };
    assert_eq!(image.width(), region.width());
    // 统计由适配器循环末尾发布，读取响应可能先到；等待统计确认而非把旧快照当零读回。
    let stats_deadline = Instant::now() + Duration::from_secs(1);
    while service.stats().pixel_readback_bytes < region.byte_len() {
        assert!(Instant::now() < stats_deadline);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let first = service.stats();
    assert_eq!(
        first.pixel_readback_bytes - before.pixel_readback_bytes,
        region.byte_len()
    );
    let again = service
        .sample(
            SampleRequest {
                previous: Some(sample.token.clone()),
                ..request.clone()
            },
            Operation::new(OperationOptions::new(Duration::from_secs(5)).unwrap()),
        )
        .await
        .unwrap();
    if matches!(again.content, SampleContent::Unchanged) {
        assert_eq!(
            service.stats().pixel_readback_bytes,
            first.pixel_readback_bytes
        );
    }
    backend.restart().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if service.sources().iter().any(|item| {
            item.id == source.id
                && item.generation > source.generation
                && item.state == SourceState::Ready
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!sample.token.snapshot.pixels.valid());
    let stats = service.stats();
    println!("capture stats: {stats:?}");
    service
        .shutdown(OperationOptions::new(Duration::from_secs(3)).unwrap())
        .await
        .unwrap();
    drop(service);
    drop(backend);
    // 同样的真实后端注入极短 GPU 时限，验证有限恢复结束为明确不可用。
    let backend = Arc::new(
        DxgiBackend::start(BackendConfig {
            gpu_timeout: Duration::from_nanos(1),
            recovery_timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );
    let service = CaptureService::start(backend, CaptureConfig::default()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if service
            .sources()
            .iter()
            .any(|source| source.state == SourceState::Unavailable)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "GPU timeout must stop retrying: {:?}",
            service.sources()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let failed = service.sources();
    assert!(
        failed
            .iter()
            .any(|source| source.state == SourceState::Unavailable)
    );
    let sequence = failed.iter().map(|source| source.generation).max();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        sequence,
        service
            .sources()
            .iter()
            .map(|source| source.generation)
            .max()
    );
    service
        .shutdown(OperationOptions::new(Duration::from_secs(3)).unwrap())
        .await
        .unwrap();
}
