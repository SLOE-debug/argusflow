//! Windows HRESULT、provider 生命周期与动作能力的内部错误边界。

use argusflow_core::{AutomationError, BackendKind};
use thiserror::Error;
use windows::{Win32::UI::Accessibility::UIA_E_ELEMENTNOTAVAILABLE, core::Error as WindowsError};

use super::native::UiaProperty;

/// UIA 原生调用的稳定操作类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaOperation {
    /// 从冻结 HWND 创建根元素。
    ElementFromHandle,
    /// 物化原生查询 condition。
    CreateCondition,
    /// 创建或配置 CacheRequest。
    BuildCache,
    /// 在限定 scope 内查询候选。
    FindAll,
    /// 读取 cached property。
    ReadProperty,
    /// 读取用于去重的 runtime id。
    ReadRuntimeId,
    /// 获取动作 pattern。
    GetPattern,
    /// 调用 InvokePattern。
    Invoke,
    /// 调用 ValuePattern。
    SetValue,
}

/// UIA 动作所需的原生 pattern。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaPattern {
    /// 语义调用。
    Invoke,
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
    /// 元素在查询和动作之间失效。
    #[error("UI Automation element became unavailable")]
    ElementUnavailable,
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
    /// 目标实例没有动作要求的 pattern。
    #[error("target does not expose required {pattern:?}Pattern")]
    RequiredPatternUnavailable {
        /// 缺失的原生动作 pattern。
        pattern: UiaPattern,
    },
    /// ValuePattern 存在但目标只读。
    #[error("target ValuePattern is read-only")]
    ReadOnlyValue,
    /// 冻结计划中的正则无法由执行器构造。
    #[error("invalid residual regular expression: {source}")]
    InvalidResidualPattern {
        /// 正则引擎返回的错误。
        #[source]
        source: regex::Error,
    },
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

    /// 按 PreparedPlan 的 fallback 约束集中映射公共错误。
    pub(crate) fn into_automation_error(self) -> AutomationError {
        let is_action_failure = matches!(
            &self,
            Self::RequiredPatternUnavailable { .. }
                | Self::ReadOnlyValue
                | Self::PropertyTypeMismatch { .. }
                | Self::InvalidResidualPattern { .. }
                | Self::InvalidRuntimeId
                | Self::NativeCallFailed {
                    operation: UiaOperation::GetPattern
                        | UiaOperation::Invoke
                        | UiaOperation::SetValue,
                    ..
                }
        );
        if is_action_failure {
            AutomationError::BackendFailed {
                backend: BackendKind::WindowsUia,
                message: self.to_string(),
            }
        } else {
            AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: self.to_string(),
            }
        }
    }
}

/// 识别 UIA provider 明确报告的 stale element HRESULT。
fn is_element_unavailable(error: &WindowsError) -> bool {
    error.code().0 as u32 == UIA_E_ELEMENTNOTAVAILABLE
}
