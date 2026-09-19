//! 装配现有微信能力，保存原始输入和本次发送证据。
use super::{
    Result,
    application::{self, ApplicationTarget},
    raw::RawRecording,
    wechat_support::{desktop::Desktop, observation::Observer},
};
use argusflow_core::OperationOptions;
use argusflow_vision::{OcrConfig, OcrEngine};
use argusflow_windows::InputService;
use std::path::Path;

pub async fn clear_own_draft(text: &str, root: &Path) -> Result<()> {
    let window = application::activate(&ApplicationTarget {
        executable: std::path::PathBuf::from(
            std::env::var_os("ProgramFiles").ok_or("缺少 ProgramFiles")?,
        )
        .join("Tencent/Weixin/Weixin.exe"),
        title: "微信".into(),
    })
    .await?;
    let mut config = OcrConfig::new(root.join(".deps"));
    config.detection_min_side = 64;
    let engine = OcrEngine::load(config).await?;
    let desktop = Desktop {
        window,
        input: InputService::new()?,
    };
    let result =
        super::wechat_once::clear_own_draft(&desktop, &mut Observer::new(engine.clone()), text)
            .await;
    let input = desktop.input.shutdown(OperationOptions::default()).await;
    let ocr = engine.shutdown(OperationOptions::default()).await;
    result?;
    input?;
    ocr?;
    Ok(())
}

pub async fn send(
    message: &str,
    root: &Path,
    run: &Path,
    step: usize,
    input: InputService,
    runtime: &argusflow_windows::UiaRuntime,
) -> Result<usize> {
    if message.trim().is_empty()
        || message.chars().count() > 80
        || message.chars().any(char::is_control)
    {
        return Err("微信消息必须是 1–80 字的单行文本".into());
    }
    let executable =
        std::path::PathBuf::from(std::env::var_os("ProgramFiles").ok_or("缺少 ProgramFiles")?)
            .join("Tencent/Weixin/Weixin.exe");
    let candidate = argusflow_windows::WindowLocator {
        title: Some("微信".into()),
        ..Default::default()
    }
    .find_unique()?
    .identity();
    super::taskbar::activate(
        runtime,
        &input,
        &candidate,
        "Appid: {6D809377-6AF0-444B-8957-A3773F02200E}\\Tencent\\Weixin\\Weixin.exe",
    )
    .await?;
    let window = application::activate(&ApplicationTarget {
        executable,
        title: "微信".into(),
    })
    .await?;
    let recording = RawRecording::start(
        &run.join(format!("wechat-{step}-input.jsonl")),
        window.process_id(),
    )?;
    let mut config = OcrConfig::new(root.join(".deps"));
    config.detection_min_side = 64;
    let engine = OcrEngine::load(config).await?;
    let desktop = Desktop { window, input };
    let mut observer = Observer::new(engine.clone());
    let result = super::wechat_once::send(
        &desktop,
        &mut observer,
        message,
        &run.join(format!("wechat-{step}")),
    )
    .await;
    let events = recording.finish();
    let ocr_cleanup = engine.shutdown(OperationOptions::default()).await;
    std::fs::write(run.join(format!("wechat-{step}.log")), match &result {
        Ok(()) => "LocalBubbleObserved：唯一运行编号、完整消息、草稿清空和稳定气泡已确认；服务器送达状态未知".into(),
        Err(error) => format!("Unconfirmed：{error}；不自动重发"),
    })?;
    result?;
    ocr_cleanup?;
    events
}
