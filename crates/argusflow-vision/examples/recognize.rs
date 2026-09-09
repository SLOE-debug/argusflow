//! cargo run -p argusflow-vision --example recognize -- .deps image.png cpu small
use argusflow_vision::{Device, ImageInput, ModelTier, OcrConfig, OcrEngine, OperationOptions};
use std::{error::Error, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() < 2 {
        return Err("usage: recognize <deps-root> <image> [cpu|cuda] [small|medium]".into());
    }
    let mut config = OcrConfig::new(&arguments[0]);
    config.device = match arguments.get(2).map(String::as_str).unwrap_or("cpu") {
        "cpu" => Device::Cpu,
        "cuda" => Device::Cuda { device_id: 0 },
        _ => return Err("unknown device".into()),
    };
    config.tier = match arguments.get(3).map(String::as_str).unwrap_or("small") {
        "small" => ModelTier::Small,
        "medium" => ModelTier::Medium,
        _ => return Err("unknown tier".into()),
    };
    let engine = OcrEngine::load(config).await?;
    let result = engine
        .recognize(ImageInput::Path(PathBuf::from(&arguments[1])))
        .await;
    engine.shutdown(OperationOptions::default()).await?;
    let result = result?;
    for block in result.blocks() {
        println!(
            "{:.4} {:?} {}",
            block.confidence(),
            block.polygon(),
            block.text()
        );
    }
    Ok(())
}
