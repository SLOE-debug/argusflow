//! 已解析唯一 UIA 元素上的 InvokePattern 与 ValuePattern 动作。

use std::collections::BTreeMap;

use serde_json::Value;
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
) -> Result<ExecutedUiaAction, UiaError> {
    match action {
        UiaActionPlan::Invoke => invoke(element),
        UiaActionPlan::SetValue { value } => set_value(element, value),
        UiaActionPlan::GetText => get_text(element),
        UiaActionPlan::GetValue => get_value(element),
    }
}

/// UIA 动作消息与结构化读取输出。
pub(crate) struct ExecutedUiaAction {
    /// 不包含完整读取值的事件说明。
    pub(crate) message: &'static str,
    /// 读取动作的值端口；写操作为空。
    pub(crate) outputs: BTreeMap<String, Value>,
}

/// 要求并调用真正的 UIA InvokePattern。
fn invoke(element: &IUIAutomationElement) -> Result<ExecutedUiaAction, UiaError> {
    // SAFETY: element 与返回的 pattern 都只在创建它们的 UIA worker apartment 中使用。
    let pattern: IUIAutomationInvokePattern =
        unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
            .map_err(|source| pattern_error(source, UiaPattern::Invoke))?;
    // SAFETY: pattern 由上面的 typed QueryInterface 成功创建，且没有跨线程传播。
    unsafe { pattern.Invoke() }
        .map_err(|source| UiaError::from_native(UiaOperation::Invoke, source))?;
    Ok(ExecutedUiaAction {
        message: "已通过 UI Automation InvokePattern 调用目标",
        outputs: BTreeMap::new(),
    })
}

/// 要求可写 ValuePattern 并直接设置 BSTR 值。
fn set_value(element: &IUIAutomationElement, value: &str) -> Result<ExecutedUiaAction, UiaError> {
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
    Ok(ExecutedUiaAction {
        message: "已通过 UI Automation ValuePattern 写入目标",
        outputs: BTreeMap::new(),
    })
}

/// 读取所有 UIA 元素都公开的 CurrentName 语义文本。
fn get_text(element: &IUIAutomationElement) -> Result<ExecutedUiaAction, UiaError> {
    // SAFETY: element 只在创建它的 UIA worker apartment 中同步使用。
    let text = unsafe { element.CurrentName() }
        .map(|value| value.to_string())
        .map_err(|source| UiaError::from_native(UiaOperation::GetText, source))?;
    Ok(ExecutedUiaAction {
        message: "已通过 UI Automation 读取目标文本",
        outputs: BTreeMap::from([("text".to_owned(), Value::String(text))]),
    })
}

/// 要求 ValuePattern 并读取当前字符串值。
fn get_value(element: &IUIAutomationElement) -> Result<ExecutedUiaAction, UiaError> {
    // SAFETY: element 与返回的 pattern 都只在创建它们的 UIA worker apartment 中使用。
    let pattern: IUIAutomationValuePattern =
        unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }
            .map_err(|source| pattern_error(source, UiaPattern::Value))?;
    // SAFETY: typed ValuePattern 仍在创建它的 UIA worker apartment 中。
    let value = unsafe { pattern.CurrentValue() }
        .map(|value| value.to_string())
        .map_err(|source| UiaError::from_native(UiaOperation::GetValue, source))?;
    Ok(ExecutedUiaAction {
        message: "已通过 UI Automation ValuePattern 读取目标值",
        outputs: BTreeMap::from([("value".to_owned(), Value::String(value))]),
    })
}

/// 把 stale element 与 pattern 缺失分开，供 executor 决定是否重试。
fn pattern_error(source: windows::core::Error, pattern: UiaPattern) -> UiaError {
    if is_pattern_unavailable(&source) {
        UiaError::RequiredPatternUnavailable { pattern }
    } else {
        UiaError::from_native(UiaOperation::GetPattern, source)
    }
}
