//! 真实剪贴板粘贴和单次发送验证，不记录计划。
use super::{
    Result, clipboard, editor_state as editor,
    notepad::Notepad,
    wechat_support::{
        bubbles,
        desktop::Desktop,
        layout::Zone,
        observation::Observer,
        send_verification::{self, SendOutcome},
    },
};
use argusflow_core::{Key, OperationOptions, ScreenPoint};
use argusflow_vision::{OcrConfig, OcrEngine};
use argusflow_windows::InputAction;
use std::{path::Path, time::Duration};
async fn pause() -> Result<()> {
    tokio::time::sleep(Duration::from_millis(1800)).await;
    Ok(())
}
pub async fn send(notepad: &Notepad, root: &Path, recipient: &str) -> Result<()> {
    send_with_services(
        notepad.runtime(),
        &notepad.input(),
        root,
        recipient,
        &clipboard::read()?,
    )
    .await
}
pub async fn send_with_services(
    _runtime: &argusflow_windows::UiaRuntime,
    input: &argusflow_windows::InputService,
    root: &Path,
    recipient: &str,
    expected: &str,
) -> Result<()> {
    // 本次授权范围仅文件传输助手；这不是推导端的接收人默认值。
    if recipient != "文件传输助手" {
        return Err("此实机 demo 未授权发送给其他联系人".into());
    }
    let target = super::application::attach(&super::application::ApplicationTarget {
        executable: std::path::PathBuf::from(
            std::env::var_os("ProgramFiles").ok_or("缺少 ProgramFiles")?,
        )
        .join("Tencent/Weixin/Weixin.exe"),
        title: "微信".into(),
    })
    .await?;
    input
        .perform(
            target.clone(),
            InputAction::ActivateWindow,
            OperationOptions::default(),
        )
        .await?;
    let desktop = Desktop {
        window: target,
        input: input.clone(),
    };
    let mut config = OcrConfig::new(root.join(".deps"));
    config.detection_min_side = 64;
    let engine = OcrEngine::load(config).await?;
    let mut observer = Observer::new(engine.clone());
    let result = async {
        let before = super::chat::open(&desktop, &mut observer, recipient).await?;
        let region = Zone::Editor.region(before.frame.bounds)?;
        if !editor::is_empty(&before, region) {
            return Err("编辑区有草稿，保留并停止".into());
        }
        let text = clipboard::read()?;
        if text != expected {
            return Err("剪贴板在打开会话期间改变，未粘贴发送".into());
        }
        if text.trim().is_empty() || text.chars().count() > 80 || text.contains(['\r', '\n']) {
            return Err("复制内容不是一条短文本".into());
        }
        let screen = before.frame.bounds.project(region)?;
        desktop
            .click(
                ScreenPoint {
                    x: screen.x() + screen.width() as i32 / 2,
                    y: screen.y() + screen.height() as i32 / 2,
                },
                before.frame.bounds,
            )
            .await?;
        pause().await?;
        // 聚焦后移开鼠标，避免屏幕采样把 I 形指针识别成草稿字符。
        desktop
            .input
            .perform(
                desktop.window.clone(),
                InputAction::Move(ScreenPoint {
                    x: before.frame.bounds.x() + before.frame.bounds.width() as i32 / 2,
                    y: before.frame.bounds.y() + 20,
                }),
                OperationOptions::default(),
            )
            .await?;
        desktop
            .input
            .perform(
                desktop.window.clone(),
                InputAction::Chord(vec![Key::Control, Key::Letter('V')]),
                OperationOptions::default(),
            )
            .await?;
        pause().await?;
        let draft = observer.refresh(&desktop).await?;
        let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        if !draft.blocks.iter().any(|b| {
            region.contains(b.rect) && compact(&b.text) == compact(&text) && b.confidence >= 0.65
        }) {
            return Err("粘贴草稿未确认，不发送".into());
        }
        let after = super::send_transition::send_once(&desktop, &mut observer, &text).await?;
        if !editor::is_empty(&after, region) {
            return Err("Enter后草稿未清空，不重发".into());
        }
        let messages = Zone::Messages.region(after.frame.bounds)?;
        if !bubbles::outgoing(&after.frame, messages).iter().any(|b| {
            send_verification::verify(&editor::without_placeholder(&after), Ok(*b), &text).ok()
                == Some(SendOutcome::LocalBubbleObserved)
        }) {
            return Err("发送气泡未确认，不重发".into());
        }
        println!("已观察到粘贴发送：{text}");
        Ok(())
    }
    .await;
    engine.shutdown(OperationOptions::default()).await?;
    result
}
