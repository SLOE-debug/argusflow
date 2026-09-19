//! 只读复核当前文件助手的新气泡，不粘贴、不发送、不修改剪贴板。
use super::{
    Result,
    wechat_support::{bubbles, desktop::Desktop, layout::Zone, observation::Observer},
};
use argusflow_core::OperationOptions;
use argusflow_windows::{InputAction, InputService, WindowLocator};
pub async fn run(root: &std::path::Path, output: &std::path::Path) -> Result<()> {
    let window = WindowLocator {
        title: Some("微信".into()),
        ..Default::default()
    }
    .find_unique()?
    .identity();
    let input = InputService::new()?;
    let mut config = argusflow_vision::OcrConfig::new(root.join(".deps"));
    config.detection_min_side = 64;
    let engine = argusflow_vision::OcrEngine::load(config).await?;
    let desktop = Desktop { window, input };
    let result = async {
        desktop.input.perform(desktop.window.clone(), InputAction::ActivateWindow, OperationOptions::default()).await?;
        let observation = Observer::new(engine.clone()).refresh(&desktop).await?;
        let header = Zone::Header.region(observation.frame.bounds)?;
        if observation.text(header, "文件传输助手", false)?.is_empty() { return Err("当前不是文件助手，不读取消息".into()); }
        let messages = Zone::Messages.region(observation.frame.bounds)?;
        let candidates = bubbles::outgoing(&observation.frame, messages);
        let bubble = candidates.iter().max_by_key(|b|b.bottom()).ok_or("没有发出气泡")?;
        let blocks = observation.blocks.iter().filter(|b|bubble.contains(b.rect)).map(|b|
            serde_json::json!({"text":b.text,"confidence":b.confidence,"rect":format!("{:?}",b.rect)})).collect::<Vec<_>>();
        let report = serde_json::json!({"bubble":format!("{bubble:?}"),"blocks":blocks,"editor_empty":super::editor_state::is_empty(&observation,Zone::Editor.region(observation.frame.bounds)?) });
        std::fs::write(output,serde_json::to_vec_pretty(&report)?)?;
        println!("{report}");
        Ok::<_,Box<dyn std::error::Error>>(())
    }.await;
    engine.shutdown(OperationOptions::default()).await?;
    desktop.input.shutdown(OperationOptions::default()).await?;
    result
}
