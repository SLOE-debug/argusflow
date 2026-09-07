//! 控制面生命周期与无输入内容的进度快照。

use serde::Serialize;

/// 是否仍在监听与是否需要保存重试必须明确区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecorderPhase {
    /// 未安装 Hook，也没有待保存的会话。
    Idle,
    /// 全局 Hook 正在监听。
    Recording,
    /// Hook 已停止，后台事件尚待排空。
    Finishing,
    /// 事件已排空，但持久化失败；再次 stop 只重试保存。
    AwaitingSave,
}

/// UI 可轮询的只读状态，不包含任何键码或字段内容。
#[derive(Debug, Clone, Serialize)]
pub struct RecorderStatus {
    /// 当前生命周期。
    pub phase: RecorderPhase,
    /// 本次录制 ID。
    pub recording_id: Option<uuid::Uuid>,
    /// 录制开始的 Unix 毫秒时间，空闲时为空。
    pub started_at_unix_ms: Option<u64>,
    /// 已完成脱敏的原始事件数。
    pub processed_events: u64,
    /// 有界队列与 Trace 容量造成的累计丢弃数。
    pub dropped_events: u64,
}
