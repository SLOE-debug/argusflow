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
#[ignore = "原生 DXGI 连续采样延迟基准，只读取 32×32 区域，不保存桌面图像"]
async fn consecutive_sampling_latency() {
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
        if let Some(info) = sampler
            .sources()
            .unwrap()
            .into_iter()
            .find(|s| s.state == SourceState::Ready)
        {
            break info;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let mut previous = None;
    let mut times = Vec::new();
    for _ in 0..16 {
        let start = Instant::now();
        let sample = sampler
            .sample(
                SampleRequest {
                    source: info.id,
                    region: PixelRect::new(0, 0, 32, 32).unwrap(),
                    quiet: Duration::from_millis(150),
                    previous,
                },
                Operation::new(OperationOptions::default()),
            )
            .await
            .unwrap_or_else(|error| {
                for history in source.history().unwrap() {
                    eprintln!(
                        "source={:?} now={:?} checked={:?} frames={:?}",
                        history.source.id,
                        source.now(),
                        history.checked,
                        history
                            .frames
                            .iter()
                            .map(|f| (f.version.revision, f.timing.frozen))
                            .collect::<Vec<_>>()
                    );
                }
                panic!("{error}");
            });
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        previous = Some(sample.token);
    }
    source
        .shutdown(Operation::new(OperationOptions::default()))
        .await
        .unwrap();
    times.sort_by(f64::total_cmp);
    println!("sampling median_ms={:.3} max_ms={:.3}", times[8], times[15]);
}

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
