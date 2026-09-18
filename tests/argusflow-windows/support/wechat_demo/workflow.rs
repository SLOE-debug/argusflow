//! 组合应用激活、增量观察、会话恢复和单次发送；不自动重放消息。
use super::{
    application,
    config::{Config, Mode},
    conversation,
    desktop::Desktop,
    observation::Observer,
    send_verification::SendOutcome,
    sending,
};
use argusflow_core::OperationOptions;
use argusflow_vision::{OcrConfig, OcrEngine};
use argusflow_windows::InputService;
use std::{error::Error, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// 装配和关闭资源，进程激活不依赖微信快捷键。
pub async fn run(config: Config) -> Result<Option<SendOutcome>> {
    let mut ocr = OcrConfig::new(&config.dependencies);
    // 小图保持接近原分辨率，避免把几十像素高的补丁强制放大到 736。
    ocr.detection_min_side = 64;
    let engine = OcrEngine::load(ocr).await?;
    let result = with_engine(&engine, config).await;
    let shutdown = engine.shutdown(OperationOptions::default()).await;
    let outcome = result?;
    shutdown?;
    Ok(outcome)
}
async fn with_engine(engine: &OcrEngine, config: Config) -> Result<Option<SendOutcome>> {
    let mut observer = Observer::new(engine.clone());
    let input = InputService::new()?;
    let result = async {
        let window = application::activate(&config.target).await?;
        let desktop = Desktop {
            input: input.clone(),
            window,
        };
        match config.mode {
            Mode::Observe => {
                observer.refresh(&desktop).await?.print();
                for _ in 0..2 {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    observer.refresh(&desktop).await?;
                }
                Ok(None)
            }
            Mode::Open => conversation::ensure_open(&desktop, &mut observer)
                .await
                .map(|_| None),
            Mode::Send(message) => sending::send(&desktop, &mut observer, &message)
                .await
                .map(Some),
        }
    }
    .await;
    let shutdown = input.shutdown(OperationOptions::default()).await;
    let outcome = result?;
    shutdown?;
    Ok(outcome)
}
