//! 已解析唯一 UIA 元素上的可调用能力与 ValuePattern 动作。

use std::collections::BTreeMap;

use argusflow_core::ActionCapability;
use serde_json::Value;
use windows::{
    Win32::UI::Accessibility::{
        IUIAutomationElement, IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern,
        IUIAutomationLegacyIAccessiblePattern, IUIAutomationValuePattern,
        UIA_ExpandCollapsePatternId, UIA_InvokePatternId,
        UIA_IsExpandCollapsePatternAvailablePropertyId, UIA_IsInvokePatternAvailablePropertyId,
        UIA_IsLegacyIAccessiblePatternAvailablePropertyId, UIA_IsValuePatternAvailablePropertyId,
        UIA_LegacyIAccessiblePatternId, UIA_PROPERTY_ID, UIA_ValuePatternId,
    },
    core::BSTR,
};

use super::{
    error::{UiaError, UiaOperation, UiaPattern, is_pattern_unavailable},
    plan::UiaActionPlan,
};

/// 通过目标实例 pattern availability 解析出的精确执行策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaActionStrategy {
    /// InvokePattern 激活。
    Invoke,
    /// ExpandCollapsePattern 展开。
    Expand,
    /// LegacyIAccessiblePattern 默认动作。
    LegacyDefaultAction,
    /// 可写 ValuePattern。
    SetValue,
}

/// 返回 prepared action 对目标实例要求的跨后端能力。
pub(crate) const fn required_capability(action: &UiaActionPlan) -> ActionCapability {
    match action {
        UiaActionPlan::Invoke => ActionCapability::Activate,
        UiaActionPlan::SetValue { .. } => ActionCapability::WriteValue,
    }
}

/// 判断语义候选能否执行当前动作，并冻结其精确 pattern 策略。
pub(crate) fn resolve_action_strategy(
    element: &IUIAutomationElement,
    action: &UiaActionPlan,
) -> Result<Option<UiaActionStrategy>, UiaError> {
    match action {
        UiaActionPlan::Invoke => resolve_invoke_strategy(element),
        UiaActionPlan::SetValue { .. } => resolve_value_strategy(element, true),
    }
}

/// 在唯一且适配后的目标上执行冻结 pattern 策略。
pub(crate) fn execute_action(
    element: &IUIAutomationElement,
    action: &UiaActionPlan,
    strategy: UiaActionStrategy,
) -> Result<ExecutedUiaAction, UiaError> {
    match (action, strategy) {
        (UiaActionPlan::Invoke, strategy) => invoke(element, strategy),
        (UiaActionPlan::SetValue { value }, UiaActionStrategy::SetValue) => {
            set_value(element, value)
        }
        _ => Err(UiaError::ActionStrategyMismatch),
    }
}

/// UIA 动作消息与结构化读取输出。
pub(crate) struct ExecutedUiaAction {
    /// 不包含完整读取值的事件说明。
    pub(crate) message: &'static str,
    /// 读取动作的值端口；写操作为空。
    pub(crate) outputs: BTreeMap<String, Value>,
}

/// 按目标公开的 UIA 能力执行语义点击。
///
/// 普通按钮使用 InvokePattern，菜单容器使用 ExpandCollapsePattern；传统 Win32/MSAA
/// 控件通过 UIA 的 LegacyIAccessiblePattern 执行默认动作。能力由 UIA 属性显式选择，
/// 不依赖鼠标坐标或应用内部命令 ID。
fn invoke(
    element: &IUIAutomationElement,
    strategy: UiaActionStrategy,
) -> Result<ExecutedUiaAction, UiaError> {
    match strategy {
        UiaActionStrategy::Invoke => {
            // SAFETY: availability 属性由同一 element/provider 返回，pattern 留在 worker apartment。
            let pattern: IUIAutomationInvokePattern =
                unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
                    .map_err(|source| pattern_error(source, UiaPattern::Click))?;
            // SAFETY: typed pattern 没有跨线程传播。
            unsafe { pattern.Invoke() }
                .map_err(|source| UiaError::from_native(UiaOperation::Invoke, source))?;
            return Ok(click_outcome("已通过 UI Automation InvokePattern 调用目标"));
        }
        UiaActionStrategy::Expand => {
            // SAFETY: availability 属性由同一 element/provider 返回，pattern 留在 worker apartment。
            let pattern: IUIAutomationExpandCollapsePattern =
                unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }
                    .map_err(|source| pattern_error(source, UiaPattern::Click))?;
            // SAFETY: typed pattern 没有跨线程传播；Expand 是菜单项的语义打开动作。
            unsafe { pattern.Expand() }
                .map_err(|source| UiaError::from_native(UiaOperation::Expand, source))?;
            return Ok(click_outcome(
                "已通过 UI Automation ExpandCollapsePattern 展开目标",
            ));
        }
        UiaActionStrategy::LegacyDefaultAction => {
            // SAFETY: LegacyIAccessiblePattern 仍是 UIA 暴露的强类型动作边界。
            let pattern: IUIAutomationLegacyIAccessiblePattern =
                unsafe { element.GetCurrentPatternAs(UIA_LegacyIAccessiblePatternId) }
                    .map_err(|source| pattern_error(source, UiaPattern::Click))?;
            // SAFETY: 默认动作由 UIA provider 定义，不发送应用私有消息或坐标点击。
            unsafe { pattern.DoDefaultAction() }.map_err(|source| {
                UiaError::from_native(UiaOperation::LegacyDefaultAction, source)
            })?;
            return Ok(click_outcome(
                "已通过 UI Automation LegacyIAccessiblePattern 调用目标",
            ));
        }
        _ => {}
    }
    Err(UiaError::ActionStrategyMismatch)
}

/// 按稳定优先级选择语义点击能力，不执行任何动作。
fn resolve_invoke_strategy(
    element: &IUIAutomationElement,
) -> Result<Option<UiaActionStrategy>, UiaError> {
    if pattern_available(element, UIA_IsInvokePatternAvailablePropertyId)? {
        Ok(Some(UiaActionStrategy::Invoke))
    } else if pattern_available(element, UIA_IsExpandCollapsePatternAvailablePropertyId)? {
        Ok(Some(UiaActionStrategy::Expand))
    } else if pattern_available(element, UIA_IsLegacyIAccessiblePatternAvailablePropertyId)? {
        Ok(Some(UiaActionStrategy::LegacyDefaultAction))
    } else {
        Ok(None)
    }
}

/// 检查 ValuePattern，并在写入动作时额外拒绝只读目标。
fn resolve_value_strategy(
    element: &IUIAutomationElement,
    require_writable: bool,
) -> Result<Option<UiaActionStrategy>, UiaError> {
    if !pattern_available(element, UIA_IsValuePatternAvailablePropertyId)? {
        return Ok(None);
    }
    if require_writable {
        // SAFETY: availability 属性已由同一 provider 返回，pattern 不离开 worker apartment。
        let pattern: IUIAutomationValuePattern =
            unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }
                .map_err(|source| pattern_error(source, UiaPattern::Value))?;
        // SAFETY: typed pattern 留在创建它的 UIA worker apartment。
        let is_read_only = unsafe { pattern.CurrentIsReadOnly() }
            .map_err(|source| UiaError::from_native(UiaOperation::GetPattern, source))?;
        if is_read_only.as_bool() {
            return Ok(None);
        }
        return Ok(Some(UiaActionStrategy::SetValue));
    }
    Ok(None)
}

/// 读取 UIA pattern availability 布尔属性，并拒绝 provider 返回的意外 VARIANT 类型。
fn pattern_available(
    element: &IUIAutomationElement,
    property: UIA_PROPERTY_ID,
) -> Result<bool, UiaError> {
    // SAFETY: property 来自 UIA 固定 availability 属性，element 留在 worker apartment。
    let value = unsafe { element.GetCurrentPropertyValue(property) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    bool::try_from(&value).map_err(|_| UiaError::PatternAvailabilityTypeMismatch { property })
}

/// 创建无输出的语义点击结果。
fn click_outcome(message: &'static str) -> ExecutedUiaAction {
    ExecutedUiaAction {
        message,
        outputs: BTreeMap::new(),
    }
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

/// 把 stale element 与 pattern 缺失分开，供 executor 决定是否重试。
fn pattern_error(source: windows::core::Error, pattern: UiaPattern) -> UiaError {
    if is_pattern_unavailable(&source) {
        UiaError::RequiredPatternUnavailable { pattern }
    } else {
        UiaError::from_native(UiaOperation::GetPattern, source)
    }
}
