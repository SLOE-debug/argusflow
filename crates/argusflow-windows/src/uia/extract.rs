//! UIA 元素到通用只读实体快照的转换。

use argusflow_core::{CoordinateSpace, EntityBounds, EntitySnapshot, EntitySource};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationValuePattern, UIA_ValuePatternId,
};

use super::error::{UiaError, UiaOperation, UiaPattern, is_pattern_unavailable};

/// 将 UIA 元素投影为跨后端统一实体快照。
pub(super) fn snapshot_element(element: &IUIAutomationElement) -> Result<EntitySnapshot, UiaError> {
    // SAFETY: element 只在创建它的 UIA worker apartment 中同步读取标准属性。
    let name = unsafe { element.CurrentName() }
        .map(|value| non_empty(value.to_string()))
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    // SAFETY: 当前矩形是所有 UIA 元素公开的标准属性，COM 对象没有跨线程。
    let rectangle = unsafe { element.CurrentBoundingRectangle() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    // SAFETY: 当前控件类型是标准 UIA property，返回裸 ID 后立即映射为文本。
    let control_type = unsafe { element.CurrentControlType() }
        .map(|value| value.0)
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    let value = read_value(element).ok().and_then(non_empty);
    Ok(EntitySnapshot {
        text: name.clone(),
        name,
        value,
        role: Some(control_type.to_string()),
        bounds: Some(EntityBounds {
            space: CoordinateSpace::ScreenPhysical,
            x: f64::from(rectangle.left),
            y: f64::from(rectangle.top),
            width: f64::from(rectangle.right - rectangle.left),
            height: f64::from(rectangle.bottom - rectangle.top),
        }),
        confidence: None,
        source: EntitySource::WindowsUia,
    })
}

/// 空字符串不作为已知实体字段发布。
fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

/// ValuePattern 是可选实体字段；不支持该 pattern 不影响其它快照字段。
fn read_value(element: &IUIAutomationElement) -> Result<String, UiaError> {
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
    // SAFETY: typed pattern 仍在创建它的 worker apartment。
    unsafe { pattern.CurrentValue() }
        .map(|value| value.to_string())
        .map_err(|source| UiaError::from_native(UiaOperation::GetValue, source))
}
