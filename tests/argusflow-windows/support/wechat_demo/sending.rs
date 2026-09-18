//! 微信单次发送的编排；操作异常或证据不足均不自动重复发送。
use super::{
    capture, conversation,
    desktop::Desktop,
    frame::Frame,
    layout::Zone,
    observation::Observer,
    recovery::ConversationState,
    send_trace,
    send_verification::{self, Reason, SendOutcome},
};
use argusflow_core::ScreenPoint;
use std::{error::Error, time::Duration};

/// 草稿确认、连续轨迹采样、OCR 验证及稳定复验依次执行。
pub async fn send(
    desktop: &Desktop,
    observer: &mut Observer,
    message: &str,
) -> Result<SendOutcome, Box<dyn Error>> {
    let snapshot = conversation::ensure_open(desktop, observer).await?;
    let bounds = snapshot.frame.bounds;
    let editor = Zone::Editor.region(bounds)?;
    if !super::editor::is_empty(&snapshot, editor) {
        let occupied: Vec<_> = snapshot
            .blocks
            .iter()
            .filter(|b| editor.intersection(b.rect).is_some())
            .map(|b| b.rect)
            .collect();
        return Err(format!("编辑区 {editor:?} 已识别到内容 {occupied:?}，保留草稿并停止").into());
    }
    let screen = bounds.project(editor)?;
    desktop
        .click(
            ScreenPoint {
                x: screen.x() + (screen.width() / 2) as i32,
                y: screen.y() + (screen.height() / 2) as i32,
            },
            bounds,
        )
        .await?;
    desktop.type_text(message).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let draft = observer.refresh(desktop).await?;
    if draft.text(editor, message, false)?.len() != 1 {
        return Err("未完整识别到待发送草稿，停止且不重输".into());
    }
    if conversation::state(&draft)? != ConversationState::Ready {
        return Err("输入后会话发生变化，不发送也不导航恢复".into());
    }
    observer
        .confirm(desktop, &draft, Zone::Header.region(bounds)?)
        .await?;
    // OCR 较慢，运动基线必须重新采集，不能使用几百毫秒前的 OCR 截图。
    let baseline = Frame::from_bgrx(bounds, capture::capture(desktop, bounds, bounds).await?)?;
    if baseline.crop(Zone::Header.region(bounds)?)?
        != draft.frame.crop(Zone::Header.region(bounds)?)?
    {
        return Err("发送前会话区域已改变，未按 Enter".into());
    }
    println!("草稿与会话确认通过，执行一次 Enter 并连续观察");
    let trace = send_trace::send_once(desktop, baseline, Zone::Messages.region(bounds)?).await?;
    let after = observer.recognize_frame(desktop, trace.frame).await?;
    let mut outcome = send_verification::verify(&after, trace.candidate, message)?;
    if let Ok(bubble) = trace.candidate
        && outcome == SendOutcome::LocalBubbleObserved
    {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let settled = observer.refresh(desktop).await?;
        if !send_verification::stable(&after.frame, &settled.frame, bubble)? {
            outcome = SendOutcome::Unconfirmed(Reason::NotStable);
        } else {
            outcome = send_verification::verify(&settled, Ok(bubble), message)?;
        }
    }
    match &outcome {
        SendOutcome::LocalBubbleObserved => println!(
            "LocalBubbleObserved：本次新生气泡、对应文本及草稿清空已确认；服务器送达状态未知"
        ),
        SendOutcome::Unconfirmed(reason) => {
            println!("Unconfirmed：{reason:?}；不是发送失败，不自动重发")
        }
    }
    Ok(outcome)
}
