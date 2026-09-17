//! 显式原生取图验收；普通测试不采集桌面。
use argusflow_capture::FrameSampler;
use argusflow_capture_contracts::*;
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::DxgiFrameSource;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "requires visible SDR desktop; explicitly captures a small region"]
async fn full_resolution_region_sampling() {
    let source = Arc::new(
        DxgiFrameSource::start(FrameConfig {
            width: 3840,
            height: 2160,
            ..Default::default()
        })
        .unwrap(),
    );
    let sampler = FrameSampler::new(source.clone()).unwrap();
    let start = Instant::now();
    let info = loop {
        if let Some(source) = sampler
            .sources()
            .unwrap()
            .into_iter()
            .find(|source| source.state == SourceState::Ready)
        {
            break source;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let request = SampleRequest {
        source: info.id,
        region: PixelRect::new(0, 0, 32, 32).unwrap(),
        quiet: Duration::ZERO,
        previous: None,
    };
    let result = sampler
        .sample(request, Operation::new(OperationOptions::default()))
        .await;
    source
        .shutdown(Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    let result = result.unwrap();
    assert_eq!(result.token.image.width(), 32);
    assert!(!result.token.snapshot.validity.valid());
}
