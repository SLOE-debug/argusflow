//! 已解析唯一 UIA 元素上的 InvokePattern 与 ValuePattern 动作。

use windows::{
    Win32::UI::Accessibility::{
        IUIAutomationElement, IUIAutomationInvokePattern, IUIAutomationValuePattern,
        UIA_InvokePatternId, UIA_ValuePatternId,
    },
    core::BSTR,
};

use super::{
    error::{UiaError, UiaOperation, UiaPattern, is_pattern_unavailable},
    plan::UiaActionPlan,
};

/// 在唯一目标上执行 prepare 阶段冻结的 pattern 策略。
pub(crate) fn execute_action(
    element: &IUIAutomationElement,
    action: &UiaActionPlan,
) -> Result<&'static str, UiaError> {
    match action {
        UiaActionPlan::Invoke => invoke(element),
        UiaActionPlan::SetValue { value } => set_value(element, value),
    }
}

/// 要求并调用真正的 UIA InvokePattern。
fn invoke(element: &IUIAutomationElement) -> Result<&'static str, UiaError> {
    // SAFETY: element 与返回的 pattern 都只在创建它们的 UIA worker apartment 中使用。
    let pattern: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
            .map_err(|source| pattern_error(source, UiaPattern::Invoke))?;
    // SAFETY: pattern 由上面的 typed QueryInterface 成功创建，且没有跨线程传播。
    unsafe { pattern.Invoke() }
        .map_err(|source| UiaError::from_native(UiaOperation::Invoke, source))?;
    Ok("已通过 UI Automation InvokePattern 调用目标")
}

/// 要求可写 ValuePattern 并直接设置 BSTR 值。
fn set_value(element: &IUIAutomationElement, value: &str) -> Result<&'static str, UiaError> {
    // SAFETY: element 与返回的 pattern 都只在创建它们的 UIA worker apartment 中使用。
    let pattern: IUIAutomationValuePattern =
        unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }
            .map_err(|source| pattern_error(source, UiaPattern::Value))?;
    // SAFETY: typed ValuePattern 仍在其创建 apartment 中，返回值由 windows crate 管理。
    let is_read_only = unsafe { pattern.CurrentIsReadOnly() }
        .map_err(|source| UiaError::from_native(UiaOperation::GetPattern, source))?;
    if is_read_only.as_bool() {
        return Err(UiaError::ReadOnlyValue);
    }
    let value = BSTR::from(value);
    // SAFETY: BSTR 在同步调用期间有效，pattern 没有离开 UIA worker thread。
    unsafe { pattern.SetValue(&value) }
        .map_err(|source| UiaError::from_native(UiaOperation::SetValue, source))?;
    Ok("已通过 UI Automation ValuePattern 写入目标")
}

/// 把 stale element 与 pattern 缺失分开，供 executor 决定是否重试。
fn pattern_error(source: windows::core::Error, pattern: UiaPattern) -> UiaError {
    if is_pattern_unavailable(&source) {
        UiaError::RequiredPatternUnavailable { pattern }
    } else {
        UiaError::from_native(UiaOperation::GetPattern, source)
    }
}
