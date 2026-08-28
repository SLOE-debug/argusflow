//! UIA 候选集合的通用 Extract 字段投影。

use std::collections::BTreeMap;

use argusflow_core::{
    ActionOutcome, ActionOutputKey, BackendKind, ExtractCardinality, FieldProjection,
    FieldProjectionSource,
};
use serde_json::{Map, Value};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationValuePattern, UIA_ValuePatternId,
};

use super::{
    error::{UiaError, UiaOperation, UiaPattern, is_pattern_unavailable},
    plan::TargetResolutionFailure,
    target_selection::ResolvedElement,
};

/// Extract 既可能遇到 provider 生命周期错误，也可能得到稳定基数结论。
pub(super) enum ExtractExecutionError {
    /// UIA 属性或 pattern 读取失败。
    Uia(UiaError),
    /// 查询结果不满足 One/Many 的目标语义。
    Resolution(TargetResolutionFailure),
}

/// 按查询顺序批量投影 UIA 元素，避免把每个列表项暴露成独立节点执行。
pub(super) fn execute_extract(
    candidates: Vec<ResolvedElement>,
    cardinality: ExtractCardinality,
    fields: &[FieldProjection],
) -> Result<ActionOutcome, ExtractExecutionError> {
    if candidates.is_empty() {
        return Err(ExtractExecutionError::Resolution(
            TargetResolutionFailure::NotFound,
        ));
    }
    if matches!(cardinality, ExtractCardinality::One) && candidates.len() != 1 {
        return Err(ExtractExecutionError::Resolution(
            TargetResolutionFailure::Ambiguous {
                matches: candidates.len(),
            },
        ));
    }
    let mut items = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        items
            .push(project_element(&candidate.element, fields).map_err(ExtractExecutionError::Uia)?);
        if matches!(cardinality, ExtractCardinality::One) {
            break;
        }
    }
    let (message, outputs) = match cardinality {
        ExtractCardinality::One => (
            "已通过 UI Automation 提取 1 个目标".to_owned(),
            BTreeMap::from([(
                ActionOutputKey::Item.as_str().to_owned(),
                Value::Object(items.remove(0)),
            )]),
        ),
        ExtractCardinality::Many => {
            let count = items.len();
            (
                format!("已通过 UI Automation 批量提取 {count} 个目标"),
                BTreeMap::from([(
                    ActionOutputKey::Items.as_str().to_owned(),
                    Value::Array(items.into_iter().map(Value::Object).collect()),
                )]),
            )
        }
    };
    Ok(ActionOutcome {
        backend: BackendKind::WindowsUia,
        message,
        outputs,
        diagnostic_evidence: Vec::new(),
    })
}

/// 从单个元素读取已在 compiler 边界验证的全部字段。
fn project_element(
    element: &IUIAutomationElement,
    fields: &[FieldProjection],
) -> Result<Map<String, Value>, UiaError> {
    fields
        .iter()
        .map(|field| {
            read_projection(element, &field.source).map(|value| (field.name.clone(), value))
        })
        .collect()
}

/// 将通用字段来源映射到稳定 UIA property 或 ValuePattern。
fn read_projection(
    element: &IUIAutomationElement,
    source: &FieldProjectionSource,
) -> Result<Value, UiaError> {
    match source {
        FieldProjectionSource::Text | FieldProjectionSource::Name => {
            current_string(element, |item| {
                // SAFETY: element 始终留在创建它的 UIA worker apartment。
                unsafe { item.CurrentName() }
            })
        }
        FieldProjectionSource::Value => read_value(element),
        FieldProjectionSource::Property { name } => read_property(element, name),
        FieldProjectionSource::Attribute { .. } => Err(UiaError::ActionStrategyMismatch),
    }
}

/// 读取 UIA ValuePattern 的当前文本值。
fn read_value(element: &IUIAutomationElement) -> Result<Value, UiaError> {
    // SAFETY: pattern 与 element 都只在当前 UIA worker apartment 内同步使用。
    let pattern: IUIAutomationValuePattern =
        unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }.map_err(|source| {
            if is_pattern_unavailable(&source) {
                UiaError::RequiredPatternUnavailable {
                    pattern: UiaPattern::Value,
                }
            } else {
                UiaError::from_native(UiaOperation::GetPattern, source)
            }
        })?;
    unsafe { pattern.CurrentValue() }
        .map(|value| Value::String(value.to_string()))
        .map_err(|source| UiaError::from_native(UiaOperation::GetValue, source))
}

/// 读取 UIA 跨控件稳定公开的常用语义属性。
fn read_property(element: &IUIAutomationElement, name: &str) -> Result<Value, UiaError> {
    match name {
        "name" => current_string(element, |item| unsafe { item.CurrentName() }),
        "automation_id" => current_string(element, |item| unsafe { item.CurrentAutomationId() }),
        "class_name" => current_string(element, |item| unsafe { item.CurrentClassName() }),
        "framework_id" => current_string(element, |item| unsafe { item.CurrentFrameworkId() }),
        "accelerator_key" => {
            current_string(element, |item| unsafe { item.CurrentAcceleratorKey() })
        }
        "access_key" => current_string(element, |item| unsafe { item.CurrentAccessKey() }),
        "enabled" => {
            current_bool(element, |item| unsafe { item.CurrentIsEnabled() }).map(Value::Bool)
        }
        "visible" => current_bool(element, |item| unsafe { item.CurrentIsOffscreen() })
            .map(|hidden| Value::Bool(!hidden)),
        "focused" => {
            current_bool(element, |item| unsafe { item.CurrentHasKeyboardFocus() }).map(Value::Bool)
        }
        _ => Err(UiaError::ActionStrategyMismatch),
    }
}

/// 拥有化 UIA BSTR 属性，并统一映射 provider 错误。
fn current_string(
    element: &IUIAutomationElement,
    read: impl FnOnce(&IUIAutomationElement) -> windows::core::Result<windows::core::BSTR>,
) -> Result<Value, UiaError> {
    read(element)
        .map(|value| Value::String(value.to_string()))
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))
}

/// 读取 UIA BOOL 属性为 JSON boolean。
fn current_bool(
    element: &IUIAutomationElement,
    read: impl FnOnce(&IUIAutomationElement) -> windows::core::Result<windows::core::BOOL>,
) -> Result<bool, UiaError> {
    read(element)
        .map(|value| value.as_bool())
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))
}
