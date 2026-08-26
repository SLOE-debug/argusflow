//! 对已发现 UIA 元素执行强类型角色与等值谓词复验。

use windows::{Win32::UI::Accessibility::IUIAutomationElement, core::BSTR};

use super::{
    condition::{control_type_id, property_id},
    error::{UiaError, UiaOperation},
    native::{
        UiaControlType, UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty,
        UiaRoleConstraint,
    },
    plan::UiaMatcherPlan,
};

/// 判断 provider 候选是否满足 Control View、角色和全部原生等值谓词。
pub(super) fn matches_current(
    element: &IUIAutomationElement,
    matcher: &UiaMatcherPlan,
) -> Result<bool, UiaError> {
    // SAFETY: element 留在创建它的 UIA worker apartment，仅同步读取当前元素类别。
    if !unsafe { element.CurrentIsControlElement() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?
        .as_bool()
        || !matches_role(element, matcher.role)?
    {
        return Ok(false);
    }
    for predicate in &matcher.pushdown {
        if !matches_predicate(element, predicate)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 比较元素当前 ControlType，并为 dialog 追加 IsDialog 约束。
fn matches_role(element: &IUIAutomationElement, role: UiaRoleConstraint) -> Result<bool, UiaError> {
    // SAFETY: element 留在创建它的 UIA worker apartment，仅同步读取当前 ControlType。
    let actual = unsafe { element.CurrentControlType() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?
        .0;
    match role {
        UiaRoleConstraint::ControlType(expected) => Ok(actual == control_type_id(expected)),
        UiaRoleConstraint::Dialog => Ok(actual == control_type_id(UiaControlType::Window)
            && read_current_value(element, UiaProperty::IsDialog)?
                == UiaNativeValue::Boolean(true)),
    }
}

/// 对单个已由 compiler 证明类型有效的等值或不等谓词求值。
fn matches_predicate(
    element: &IUIAutomationElement,
    predicate: &UiaNativePredicate,
) -> Result<bool, UiaError> {
    let actual = read_current_value(element, predicate.property)?;
    Ok(match &predicate.comparison {
        UiaNativeComparison::Equal(expected) => actual == *expected,
        UiaNativeComparison::NotEqual(expected) => actual != *expected,
    })
}

/// 把当前 UIA property 转换回 compiler 使用的同一强类型值域。
fn read_current_value(
    element: &IUIAutomationElement,
    property: UiaProperty,
) -> Result<UiaNativeValue, UiaError> {
    // SAFETY: property id 来自封闭映射，element 留在当前 UIA worker apartment。
    let value = unsafe { element.GetCurrentPropertyValue(property_id(property)) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
    match property {
        UiaProperty::Name
        | UiaProperty::AutomationId
        | UiaProperty::ClassName
        | UiaProperty::AcceleratorKey
        | UiaProperty::AccessKey
        | UiaProperty::FrameworkId
        | UiaProperty::Value => BSTR::try_from(&value)
            .map(|value| UiaNativeValue::Text(value.to_string()))
            .map_err(|_| UiaError::PropertyTypeMismatch { property }),
        UiaProperty::IsEnabled
        | UiaProperty::IsOffscreen
        | UiaProperty::HasKeyboardFocus
        | UiaProperty::IsSelected
        | UiaProperty::IsDialog => bool::try_from(&value)
            .map(UiaNativeValue::Boolean)
            .map_err(|_| UiaError::PropertyTypeMismatch { property }),
        UiaProperty::ToggleState => i32::try_from(&value)
            .map(UiaNativeValue::Integer)
            .map_err(|_| UiaError::PropertyTypeMismatch { property }),
    }
}
