//! 将轨迹、草稿消失与局部 OCR 组合为本地 UI 证据，不宣称网络送达。
use super::{
    bubbles, conversation_state, frame::Frame, layout::Zone, message_tracking::Uncertainty,
    recovery::ConversationState, snapshot::Observation,
};
use argusflow_capture_contracts::PixelRect;
use std::error::Error;

/// 屏幕观察的结果域不包含 Delivered；需要应用回执才能增加该状态。
#[derive(Debug, PartialEq, Eq)]
pub enum SendOutcome {
    /// 只证明本窗口本次新生气泡已呈现对应文本，不能代替服务端 ACK。
    LocalBubbleObserved,
    /// 证据不足不等于失败；调用者必须保留“不重发”的约束。
    Unconfirmed(Reason),
}
/// 将各证据边界保留为类型，方便后续 workflow 消费。
#[derive(Debug, PartialEq, Eq)]
pub enum Reason {
    /// 几何轨迹不成立。
    Tracking(Uncertainty),
    /// 会话不再是预期目标。
    ContextChanged,
    /// 输入区仍识别到内容，不能假定草稿已经清空。
    DraftPresent,
    /// 文本不完全一致、置信度不足或不在完整新气泡内。
    TextMismatch,
    /// 可能有发送中/发送失败图标，保守拒绝正向确认。
    StatusNotClear,
    /// 新气泡在稳定复验时消失或改变。
    NotStable,
}

/// 同一帧的气泡、文字和输入框必须联合验证，旧消息的次数不参与判定。
pub fn verify(
    snapshot: &Observation,
    candidate: Result<PixelRect, Uncertainty>,
    message: &str,
) -> Result<SendOutcome, Box<dyn Error>> {
    let bubble = match candidate {
        Ok(bubble) => bubble,
        Err(reason) => return Ok(SendOutcome::Unconfirmed(Reason::Tracking(reason))),
    };
    if conversation_state::state(snapshot)? != ConversationState::Ready {
        return Ok(SendOutcome::Unconfirmed(Reason::ContextChanged));
    }
    let editor = Zone::Editor.region(snapshot.frame.bounds)?;
    if !super::editor::is_empty(snapshot, editor) {
        return Ok(SendOutcome::Unconfirmed(Reason::DraftPresent));
    }
    let text = snapshot.text(bubble, message, false)?;
    if text.len() != 1 {
        return Ok(SendOutcome::Unconfirmed(Reason::TextMismatch));
    }
    if !bubbles::status_slot_clear(&snapshot.frame, bubble) {
        return Ok(SendOutcome::Unconfirmed(Reason::StatusNotClear));
    }
    Ok(SendOutcome::LocalBubbleObserved)
}

/// 复验气泡内容未变且状态槽持续为空；不以“等待够久”推断网络成功。
pub fn stable(before: &Frame, after: &Frame, bubble: PixelRect) -> Result<bool, Box<dyn Error>> {
    Ok(before.bounds == after.bounds
        && before.crop(bubble)? == after.crop(bubble)?
        && bubbles::status_slot_clear(after, bubble))
}
