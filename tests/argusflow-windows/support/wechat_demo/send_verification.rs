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
    if !bubble_text_matches(snapshot, bubble, message) {
        return Ok(SendOutcome::Unconfirmed(Reason::TextMismatch));
    }
    if !bubbles::status_slot_clear(&snapshot.frame, bubble) {
        return Ok(SendOutcome::Unconfirmed(Reason::StatusNotClear));
    }
    Ok(SendOutcome::LocalBubbleObserved)
}

/// OCR 可将同一行按词拆块；只在同一气泡内按横坐标重建，歧义布局拒绝确认。
fn bubble_text_matches(snapshot: &Observation, bubble: PixelRect, message: &str) -> bool {
    let mut blocks: Vec<_> = snapshot
        .blocks
        .iter()
        .filter(|b| bubble.contains(b.rect))
        .collect();
    if blocks.is_empty()
        || blocks
            .iter()
            .any(|b| b.confidence < 0.65 || b.text.is_empty())
    {
        return false;
    }
    blocks.sort_by_key(|b| b.rect.x());
    let top = blocks.iter().map(|b| b.rect.y()).max().unwrap();
    let bottom = blocks.iter().map(|b| b.rect.bottom()).min().unwrap();
    if top >= bottom
        || blocks
            .windows(2)
            .any(|pair| pair[0].rect.right() > pair[1].rect.x())
    {
        return false;
    }
    // OCR 不可靠地保留视觉空格；仅归一化空白，不容忍字符替换、缺字或子串。
    // 原始消息仍由发送前的剪贴板/草稿校验守护，这里只确认新气泡的可见正文。
    let actual: String = blocks
        .iter()
        .flat_map(|block| block.text.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    let expected: String = message.chars().filter(|c| !c.is_whitespace()).collect();
    !expected.is_empty() && actual == expected
}

/// 复验气泡内容未变且状态槽持续为空；不以“等待够久”推断网络成功。
pub fn stable(before: &Frame, after: &Frame, bubble: PixelRect) -> Result<bool, Box<dyn Error>> {
    Ok(before.bounds == after.bounds
        && before.crop(bubble)? == after.crop(bubble)?
        && bubbles::status_slot_clear(after, bubble))
}
