use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PreparedAutomationTarget;

/// UI 节点为了满足当前动作前置条件而采用的目标等待模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetWaitMode {
    /// 只执行一次冻结计划，目标未出现时立即返回失败。
    None,
    /// 在一个共享截止时间内重复执行同一冻结计划。
    Bounded,
}

/// 节点级目标就绪等待策略。
///
/// 该策略描述动作的执行预算，不属于 `AutomationTarget` 的定位语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetWaitPolicy {
    /// 是否允许在目标暂时不存在时继续等待。
    pub mode: TargetWaitMode,
    /// `Bounded` 模式下整份 PreparedPlan 共享的总等待时长。
    pub timeout_ms: u64,
    /// 两次完整 PreparedPlan 尝试之间的轮询间隔。
    pub poll_interval_ms: u64,
}

impl TargetWaitPolicy {
    /// 创建不进行目标等待的单次执行策略。
    pub const fn none() -> Self {
        Self {
            mode: TargetWaitMode::None,
            timeout_ms: 0,
            poll_interval_ms: 0,
        }
    }

    /// 创建使用显式毫秒预算的有界等待策略。
    pub const fn bounded(timeout_ms: u64, poll_interval_ms: u64) -> Self {
        Self {
            mode: TargetWaitMode::Bounded,
            timeout_ms,
            poll_interval_ms,
        }
    }
}

impl Default for TargetWaitPolicy {
    fn default() -> Self {
        Self::none()
    }
}

/// UI 节点除动作语义以外的执行策略。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiExecutionPolicy {
    /// 等待当前 operation 自身目标满足动作要求的策略。
    pub target_wait: TargetWaitPolicy,
}

/// Runtime 传给动作分发器的节点级执行选项。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActionExecutionOptions {
    /// 只对 `TargetNotFound` 生效的统一目标等待策略。
    pub target_wait: TargetWaitPolicy,
    /// Runtime 已冻结的目标；后端不得重新解析原始表达式或自行物化视觉目标。
    pub prepared_target: Option<PreparedAutomationTarget>,
    /// 仅用于关联诊断 artifact 的 Run/Node 身份，不改变动作执行语义。
    pub trace_context: Option<RunTraceContext>,
}

/// Runtime 传给观察分发器的最小执行选项。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservationExecutionOptions {
    /// 关联本次检查产生的 OCR、Scene 和查询 Trace，不改变观察语义。
    pub trace_context: Option<RunTraceContext>,
}

/// 跨 Runtime、Agent、Vision 传递的最小 Run Trace 关联身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTraceContext {
    /// 与 WorkflowEngine 本次运行完全一致的 UUID。
    pub run_id: Uuid,
    /// 当前扁平执行节点 ID。
    pub node_id: String,
    /// 当前节点在本次运行中的执行序号，循环重复执行时不会复用。
    pub node_sequence: u64,
}
