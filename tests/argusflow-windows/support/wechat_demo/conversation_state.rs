//! 根据同一全窗口快照判断会话，图标和窗口控制按钮不能当作会话标题。
use super::{
    layout::Zone,
    recovery::ConversationState,
    snapshot::Observation,
    spatial::{self, Direction},
};
use argusflow_capture_contracts::PixelRect;
use std::{error::Error, num::NonZeroUsize};
/// 示例的固定消息接收对象。
pub const ASSISTANT: &str = "文件传输助手";

/// 搜索锚点右侧完整匹配目标标题才认为会话就绪。
pub fn state(snapshot: &Observation) -> Result<ConversationState, Box<dyn Error>> {
    let bounds = snapshot.frame.bounds;
    let header = Zone::Header.region(bounds)?;
    let searches = snapshot.text(header, "搜索", true)?;
    let anchor = spatial::top_left(&searches).ok_or("未识别到搜索锚点，无法确认微信主界面")?;
    let titles = snapshot.text(header, ASSISTANT, false)?;
    if spatial::nearest(
        &titles,
        anchor,
        Direction::Right,
        NonZeroUsize::MIN,
        header.height() / 2,
    )
    .is_some()
    {
        return Ok(ConversationState::Ready);
    }
    let x = bounds.width() * 35 / 100;
    let right_header = PixelRect::new(x, header.y(), bounds.width() - x, header.height())?;
    // 使用完整位于标题带内的文字，排除最大化/关闭图标的识别框跨入标题带的情况。
    let has_title = snapshot
        .blocks
        .iter()
        .any(|b| b.confidence >= 0.65 && right_header.contains(b.rect));
    Ok(
        if !has_title && snapshot.is_empty(Zone::Messages.region(bounds)?) {
            ConversationState::Empty
        } else {
            ConversationState::Other
        },
    )
}
