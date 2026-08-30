use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PreparedAutomationTarget, VisualQueryExpr};

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiExecutionPolicy {
    /// 等待当前 operation 自身目标满足动作要求的策略。
    pub target_wait: TargetWaitPolicy,
    /// 动作完成后观察视觉后置条件的独立截止策略。
    pub postcondition_wait: TargetWaitPolicy,
    /// 高风险输入动作完成后必须满足的视觉新事实。
    #[serde(default)]
    pub postcondition: Option<UiPostcondition>,
}

impl Default for UiExecutionPolicy {
    fn default() -> Self {
        Self {
            target_wait: TargetWaitPolicy::none(),
            postcondition_wait: default_postcondition_wait(),
            postcondition: None,
        }
    }
}

/// 为需要视觉确认的输入动作提供一个有限且独立的观察预算。
fn default_postcondition_wait() -> TargetWaitPolicy {
    TargetWaitPolicy::bounded(5_000, 150)
}

/// UI 输入动作的可验证后置条件；它描述动作后的新事实而非旧文本存在性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiPostcondition {
    /// 要求动作前后保持同一视觉上下文，且目标文字实例数量严格增加。
    NewText {
        /// 需要在动作前后做 scene delta 比较的视觉查询。
        query: VisualQueryExpr,
        /// 动作前后都必须唯一命中且保持在原位置的上下文查询。
        stable_context: Vec<VisualQueryExpr>,
    },
    /// 要求动作完成后的新鲜画面中唯一存在目标文字。
    TextPresent {
        /// 只在动作完成后求值的视觉查询。
        query: VisualQueryExpr,
    },
}

/// Runtime 传给动作分发器的节点级执行选项。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActionExecutionOptions {
    /// 只对 `TargetNotFound` 生效的统一目标等待策略。
    pub target_wait: TargetWaitPolicy,
    /// 视觉后置条件自己的观察等待策略，不占用目标物化预算。
    pub postcondition_wait: TargetWaitPolicy,
    /// Runtime 已冻结的目标；后端不得重新解析原始表达式或自行物化视觉目标。
    pub prepared_target: Option<PreparedAutomationTarget>,
    /// 由 Runtime 解析的不可持久化动作后置条件。
    pub postcondition: Option<crate::PreparedVisualPostcondition>,
    /// 仅用于关联诊断 artifact 的 Run/Node 身份，不改变动作执行语义。
    pub trace_context: Option<RunTraceContext>,
}

/// 跨 Runtime、Agent、Vision 传递的最小 Run Trace 关联身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTraceContext {
    /// 与 WorkflowEngine 本次运行完全一致的 UUID。
    pub run_id: Uuid,
    /// 当前扁平执行节点 ID。
    pub node_id: String,
}
