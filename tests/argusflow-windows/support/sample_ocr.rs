//! 显式运行时枚举来源，或识别指定来源的完整物理像素区域；不保存图像。
use argusflow_capture::{CaptureConfig, CaptureService};
use argusflow_capture_contracts::{BackendConfig, PixelRect, SourceId, SourceState};
use argusflow_core::OperationOptions;
use argusflow_vision::{OcrConfig, OcrEngine, SampledOcr};
use argusflow_windows::DxgiBackend;
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 1 && arguments.len() != 6 {
        return Err("usage: sample_ocr <deps-root> [source-id x y width height]".into());
    }
    let backend = Arc::new(DxgiBackend::start(BackendConfig::default())?);
    let service = CaptureService::start(backend, CaptureConfig::default())?;
    let result = execute(&service, &arguments).await;
    service
        .shutdown(OperationOptions::new(Duration::from_secs(3))?)
        .await?;
    result
}
async fn execute(service: &CaptureService, arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if service
            .sources()
            .iter()
            .any(|source| source.state == SourceState::Ready)
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("no ready SDR source before deadline".into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    for source in service.sources() {
        println!(
            "source={} bounds={:?} state={:?}",
            source.id.0, source.bounds, source.state
        );
    }
    if arguments.len() == 1 {
        return Ok(());
    }
    let source = SourceId(arguments[1].parse()?);
    let region = PixelRect::new(
        arguments[2].parse()?,
        arguments[3].parse()?,
        arguments[4].parse()?,
        arguments[5].parse()?,
    )?;
    let engine = OcrEngine::load(OcrConfig::new(&arguments[0])).await?;
    let sampler = SampledOcr::new(Arc::new(service.clone()), engine.clone());
    let result = sampler
        .recognize(
            source,
            region,
            OperationOptions::new(Duration::from_secs(10))?,
        )
        .await;
    sampler.clear_cache();
    engine.shutdown(OperationOptions::default()).await?;
    let result = result?;
    println!(
        "version={:?} bounds={:?} reused={}",
        result.version(),
        result.bounds(),
        result.reused()
    );
    println!("{}", result.result().text());
    println!("stats={:?}", service.stats());
    Ok(())
}
