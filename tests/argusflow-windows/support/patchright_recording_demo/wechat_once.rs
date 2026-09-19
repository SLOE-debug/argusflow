//! demo 的唯一消息验证：只按一次 Enter，通过新增完整文本确认，不重发。
use super::{
    Result,
    evidence::save_frame,
    wechat_support::{
        bubbles, conversation, conversation_state,
        desktop::Desktop,
        editor,
        layout::Zone,
        observation::Observer,
        recovery::ConversationState,
        send_verification::{self, SendOutcome},
    },
};
use argusflow_core::{Key, ScreenPoint};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub async fn send(
    desktop: &Desktop,
    observer: &mut Observer,
    message: &str,
    evidence: &Path,
) -> Result<()> {
    let before = normalized(conversation::ensure_open(desktop, observer).await?);
    let original_message = message;
    let message = compact(message);
    let message = message.as_str();
    let bounds = before.frame.bounds;
    let editor_region = Zone::Editor.region(bounds)?;
    let messages = Zone::Messages.region(bounds)?;
    if !editor::is_empty(&before, editor_region) {
        return Err("编辑区有草稿，保留并停止".into());
    }
    if !before.text(messages, message, false)?.is_empty() {
        return Err("消息已在当前会话可见，拒绝重复发送".into());
    }
    let screen = bounds.project(editor_region)?;
    desktop
        .click(
            ScreenPoint {
                x: screen.x() + (screen.width() / 2) as i32,
                y: screen.y() + (screen.height() / 2) as i32,
            },
            bounds,
        )
        .await?;
    desktop.type_text(original_message).await?;
    let started = Instant::now();
    let draft = loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let draft = normalized(observer.refresh(desktop).await?);
        save_frame(&draft.frame, &evidence.with_extension("before.bmp"))?;
        if draft.text(editor_region, message, false)?.len() == 1 {
            break draft;
        }
        if started.elapsed() > Duration::from_secs(5) {
            let texts: Vec<_> = draft
                .blocks
                .iter()
                .filter(|b| editor_region.intersection(b.rect).is_some())
                .map(|b| (&b.text, b.confidence))
                .collect();
            return Err(format!("完整草稿未识别，不发送：{texts:?}").into());
        }
    };
    if conversation_state::state(&draft)? != ConversationState::Ready {
        return Err("会话改变，不发送".into());
    }
    observer
        .confirm(desktop, &draft, Zone::Header.region(bounds)?)
        .await?;
    desktop.key(Key::Enter).await?;
    let started = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let after = normalized(observer.refresh(desktop).await?);
        save_frame(&after.frame, &evidence.with_extension("after.bmp"))?;
        let matches = after.text(messages, message, false)?;
        if matches.len() == 1 {
            for bubble in bubbles::outgoing(&after.frame, messages) {
                if send_verification::verify(&after, Ok(bubble), message)?
                    == SendOutcome::LocalBubbleObserved
                {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let settled = normalized(observer.refresh(desktop).await?);
                    if send_verification::stable(&after.frame, &settled.frame, bubble)?
                        && send_verification::verify(&settled, Ok(bubble), message)?
                            == SendOutcome::LocalBubbleObserved
                    {
                        save_frame(&settled.frame, &evidence.with_extension("after.bmp"))?;
                        println!("LocalBubbleObserved：{message}");
                        return Ok(());
                    }
                }
            }
        }
        if started.elapsed() > Duration::from_secs(8) {
            return Err("发送后 UI 结果未确认，不重发".into());
        }
    }
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn normalized(
    mut observation: super::wechat_support::snapshot::Observation,
) -> super::wechat_support::snapshot::Observation {
    // OCR 可能在同一文字块中插入换行；只规范空白，不放宽文字或置信度匹配。
    observation.blocks = std::sync::Arc::new(
        observation
            .blocks
            .iter()
            .cloned()
            .map(|mut block| {
                block.text = compact(&block.text);
                block
            })
            .collect(),
    );
    observation
}

pub async fn clear_own_draft(
    desktop: &Desktop,
    observer: &mut Observer,
    expected: &str,
) -> Result<()> {
    let draft = normalized(conversation::ensure_open(desktop, observer).await?);
    let region = Zone::Editor.region(draft.frame.bounds)?;
    if draft.text(region, &compact(expected), false)?.len() != 1 {
        return Err("草稿不是本次失败 demo 的完整文本，拒绝清理".into());
    }
    let screen = draft.frame.bounds.project(region)?;
    desktop
        .click(
            ScreenPoint {
                x: screen.x() + (screen.width() / 2) as i32,
                y: screen.y() + (screen.height() / 2) as i32,
            },
            draft.frame.bounds,
        )
        .await?;
    desktop
        .input
        .perform(
            desktop.window.clone(),
            argusflow_windows::InputAction::Chord(vec![Key::Control, Key::Letter('A')]),
            argusflow_core::OperationOptions::default(),
        )
        .await?;
    desktop.key(Key::Backspace).await?;
    let started = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let after = observer.refresh(desktop).await?;
        if editor::is_empty(&after, region) {
            break;
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err("demo 草稿清理未确认".into());
        }
    }
    Ok(())
}
