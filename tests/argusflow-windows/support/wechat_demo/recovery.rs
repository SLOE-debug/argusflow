//! 会话恢复策略：已经就绪不点击，空白只允许一次重新点击，发送不属于重试范围。

/// 每次根据新快照判定，不能沿用点击前的会话状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationState {
    /// 搜索右侧存在完整目标标题。
    Ready,
    /// 右侧没有标题，也没有消息文字。
    Empty,
    /// 其他会话或无法确认的页面。
    Other,
}
/// 只决定导航动作，不拥有键鼠或发送能力。
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// 保持当前会话，不点击已选联系人。
    Keep,
    /// 从新快照定位联系人，首次点击。
    Open,
    /// 点击后空白，从新快照重新定位并恢复一次。
    Recover,
    /// 超过恢复预算或页面无法确认。
    Stop,
}
/// 次数只计算已执行的联系人点击，不把观察轮询算成点击。
pub fn decide(state: ConversationState, clicks: u8) -> Decision {
    match (state, clicks) {
        (ConversationState::Ready, _) => Decision::Keep,
        (_, 0) => Decision::Open,
        (ConversationState::Empty, 1) => Decision::Recover,
        _ => Decision::Stop,
    }
}
