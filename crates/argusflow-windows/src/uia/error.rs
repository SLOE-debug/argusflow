//! Windows HRESULT、provider 生命周期与动作能力的内部错误边界。

use argusflow_core::{AutomationError, BackendKind};
use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{
            CO_E_SERVER_STOPPING, RPC_E_CONNECTION_TERMINATED, RPC_E_DISCONNECTED,
            RPC_E_SERVER_DIED, RPC_E_SERVER_DIED_DNE, RPC_E_TIMEOUT,
        },
        UI::Accessibility::{
            UIA_E_ELEMENTNOTAVAILABLE, UIA_E_INVALIDOPERATION, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT,
        },
    },
    core::Error as WindowsError,
};

use super::native::UiaProperty;

/// UIA 原生调用的稳定操作类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaOperation {
    /// 显式配置 IUIAutomation2 provider 超时。
    ConfigureTimeouts,
    /// 从冻结 HWND 创建根元素。
    ElementFromHandle,
    /// 获取进程级 provider fragment 查询使用的桌面根元素。
    GetDesktopRoot,
    /// 物化原生查询 condition。
    CreateCondition,
    /// 创建或配置 CacheRequest。
    BuildCache,
    /// 在限定 scope 内查询候选。
    FindAll,
    /// 在限定 scope 内查询第一个候选。
    FindFirst,
    /// 通过 RawView TreeWalker 有界导航 provider 树。
    NavigateTree,
    /// 读取 cached property。
    ReadProperty,
    /// 读取用于去重的 runtime id。
    ReadRuntimeId,
    /// 获取动作 pattern。
    GetPattern,
    /// 调用 InvokePattern。
    Invoke,
    /// 调用 ExpandCollapsePattern 展开目标。
    Expand,
    /// 调用 LegacyIAccessiblePattern 默认动作。
    LegacyDefaultAction,
    /// 调用 ValuePattern。
    SetValue,
    /// 读取 ValuePattern 当前值。
    GetValue,
}

/// UIA 请求预算限制的强类型资源类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaBudgetResource {
    /// 进程查询与有界 TreeWalker 累计观察到的 provider 节点数。
    TraversalNodes,
    /// Child/Descendant 关系展开的累计根元素数。
    RelationRoots,
}

/// UIA 动作所需的原生 pattern。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaPattern {
    /// 可通过 Invoke、ExpandCollapse 或 LegacyIAccessible 执行的语义点击。
    Click,
    /// 直接值写入。
    Value,
}

/// 不向 UIA 模块外泄漏 HRESULT 细节的内部错误。
#[derive(Debug, Error)]
pub(crate) enum UiaError {
    /// prepare 冻结的 HWND 已关闭或被其它进程复用。
    #[error("prepared window is unavailable: {message}")]
    WindowUnavailable {
        /// 句柄校验失败原因。
        message: String,
    },
    /// Windows PID 无法表示为 UIA ProcessId property 要求的 VT_I4。
    #[error("process id {process_id} cannot be represented by UI Automation")]
    InvalidProcessId {
        /// prepare 阶段从 HWND 读取的原始 Windows PID。
        process_id: u32,
    },
    /// 元素在查询和动作之间失效。
    #[error("UI Automation element became unavailable")]
    ElementUnavailable,
    /// 请求排队或执行已经超过 ArgusFlow 层截止时刻。
    #[error("UI Automation execution deadline was exceeded")]
    ExecutionDeadlineExceeded,
    /// 查询宽度超过稳定资源限制，拒绝继续扫描 provider 结果。
    #[error("UI Automation {resource:?} budget exceeded: observed {observed}, limit {limit}")]
    BudgetExceeded {
        /// 被耗尽的资源类别。
        resource: UiaBudgetResource,
        /// runtime 配置的稳定上限。
        limit: usize,
        /// 当前请求已经观察到的累计数量。
        observed: usize,
    },
    /// provider 返回了无法作为数组长度处理的负候选数。
    #[error("UI Automation provider returned invalid candidate count {count}")]
    InvalidCandidateCount {
        /// IUIAutomationElementArray::Length 返回的原始值。
        count: i32,
    },
    /// 某次原生 UIA 调用失败。
    #[error("UI Automation operation {operation:?} failed: {source}")]
    NativeCallFailed {
        /// 失败调用的稳定类别。
        operation: UiaOperation,
        /// Windows 返回的原始错误。
        #[source]
        source: WindowsError,
    },
    /// provider 返回的 VARIANT 与 compiler 证明的属性类型不一致。
    #[error("UI Automation property {property:?} returned an unexpected value type")]
    PropertyTypeMismatch {
        /// 类型不一致的 UIA 属性。
        property: UiaProperty,
    },
    /// provider 的 pattern availability 属性没有返回布尔值。
    #[error(
        "UI Automation pattern availability property {property:?} returned an unexpected value type"
    )]
    PatternAvailabilityTypeMismatch {
        /// UIA 固定 pattern availability property id。
        property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
    },
    /// 目标实例没有动作要求的 pattern。
    #[error("target does not expose a supported {pattern:?} UI Automation capability")]
    RequiredPatternUnavailable {
        /// 缺失的原生动作 pattern。
        pattern: UiaPattern,
    },
    /// ValuePattern 存在但目标只读。
    #[error("target ValuePattern is read-only")]
    ReadOnlyValue,
    /// resolved strategy 与 prepared action 不一致，表示内部执行不变量被破坏。
    #[error("resolved UI Automation action strategy does not match the prepared action")]
    ActionStrategyMismatch,
    /// provider 返回了无效的 runtime id SAFEARRAY。
    #[error("target returned an invalid UI Automation runtime id")]
    InvalidRuntimeId,
}

impl UiaError {
    /// 保留 element unavailable 语义，其余 HRESULT 记录具体操作。
    pub(crate) fn from_native(operation: UiaOperation, source: WindowsError) -> Self {
        if is_element_unavailable(&source) {
            Self::ElementUnavailable
        } else {
            Self::NativeCallFailed { operation, source }
        }
    }

    /// 判断错误是否允许用同一份冻结计划重新解析一次目标。
    pub(crate) const fn is_element_unavailable(&self) -> bool {
        matches!(self, Self::ElementUnavailable)
    }

    /// 按 HRESULT 与稳定语义集中映射公共错误；未知原生错误默认不可回退。
    pub(crate) fn into_automation_error(self) -> AutomationError {
        if self.is_backend_unavailable() {
            AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: self.to_string(),
            }
        } else {
            AutomationError::BackendFailed {
                backend: BackendKind::WindowsUia,
                message: self.to_string(),
            }
        }
    }

    /// 只有窗口/provider 生命周期或 deadline 错误允许 PreparedPlan fallback。
    fn is_backend_unavailable(&self) -> bool {
        match self {
            Self::WindowUnavailable { .. }
            | Self::ElementUnavailable
            | Self::ExecutionDeadlineExceeded => true,
            Self::NativeCallFailed { source, .. } => is_transient_provider_failure(source),
            Self::BudgetExceeded { .. }
            | Self::InvalidProcessId { .. }
            | Self::InvalidCandidateCount { .. }
            | Self::PropertyTypeMismatch { .. }
            | Self::PatternAvailabilityTypeMismatch { .. }
            | Self::RequiredPatternUnavailable { .. }
            | Self::ReadOnlyValue
            | Self::ActionStrategyMismatch
            | Self::InvalidRuntimeId => false,
        }
    }
}

/// 识别 UIA provider 明确报告的 stale element HRESULT。
fn is_element_unavailable(error: &WindowsError) -> bool {
    error.code().0 as u32 == UIA_E_ELEMENTNOTAVAILABLE
}

/// 识别 UIA/RPC 明确报告的 provider 环境中断；其它 HRESULT 一律视为实现失败。
fn is_transient_provider_failure(error: &WindowsError) -> bool {
    let code = error.code().0 as u32;
    code == UIA_E_TIMEOUT
        || code == RPC_E_TIMEOUT.0 as u32
        || code == RPC_E_DISCONNECTED.0 as u32
        || code == RPC_E_CONNECTION_TERMINATED.0 as u32
        || code == RPC_E_SERVER_DIED.0 as u32
        || code == RPC_E_SERVER_DIED_DNE.0 as u32
        || code == CO_E_SERVER_STOPPING.0 as u32
        || code == HRESULT_RPC_SERVER_UNAVAILABLE
}

/// 判断 GetCurrentPatternAs 是否明确表示目标实例没有所需 pattern。
pub(crate) fn is_pattern_unavailable(error: &WindowsError) -> bool {
    let code = error.code().0 as u32;
    code == UIA_E_NOTSUPPORTED || code == UIA_E_INVALIDOPERATION
}

/// `HRESULT_FROM_WIN32(RPC_S_SERVER_UNAVAILABLE)`；windows crate 未直接导出该 HRESULT。
const HRESULT_RPC_SERVER_UNAVAILABLE: u32 = 0x8007_06BA;

#[cfg(test)]
mod tests {
    use argusflow_core::AutomationError;
    use windows::{
        Win32::{
            Foundation::E_INVALIDARG,
            UI::Accessibility::{UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT},
        },
        core::{Error as WindowsError, HRESULT},
    };

    use super::{UiaError, UiaOperation};

    /// UIA timeout 表示 provider 环境不可用，允许冻结计划尝试其它 backend。
    #[test]
    fn timeout_hresult_maps_to_backend_unavailable() {
        let error = UiaError::from_native(
            UiaOperation::FindAll,
            WindowsError::from_hresult(HRESULT(UIA_E_TIMEOUT as i32)),
        );

        assert!(matches!(
            error.into_automation_error(),
            AutomationError::BackendUnavailable { .. }
        ));
    }

    /// 编译器/native IR 不一致不得被伪装成运行环境故障。
    #[test]
    fn invalid_argument_maps_to_backend_failed() {
        let error = UiaError::from_native(
            UiaOperation::CreateCondition,
            WindowsError::from_hresult(E_INVALIDARG),
        );

        assert!(matches!(
            error.into_automation_error(),
            AutomationError::BackendFailed { .. }
        ));
    }

    /// UIA_E_NOTSUPPORTED 只有动作 pattern 获取点会转换为 pattern 缺失。
    #[test]
    fn unsupported_query_operation_maps_to_backend_failed() {
        let error = UiaError::from_native(
            UiaOperation::FindAll,
            WindowsError::from_hresult(HRESULT(UIA_E_NOTSUPPORTED as i32)),
        );

        assert!(matches!(
            error.into_automation_error(),
            AutomationError::BackendFailed { .. }
        ));
    }
}
