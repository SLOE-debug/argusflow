//! `UiaMatcherPlan` 到原生 `IUIAutomationCondition` 的物化。

use windows::Win32::{
    System::Variant::VARIANT,
    UI::Accessibility::{
        IUIAutomation, IUIAutomationCondition, UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId,
        UIA_CheckBoxControlTypeId, UIA_ClassNamePropertyId, UIA_ComboBoxControlTypeId,
        UIA_ControlTypePropertyId, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
        UIA_HasKeyboardFocusPropertyId, UIA_HyperlinkControlTypeId, UIA_ImageControlTypeId,
        UIA_IsDialogPropertyId, UIA_IsEnabledPropertyId, UIA_IsOffscreenPropertyId,
        UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_MenuControlTypeId,
        UIA_MenuItemControlTypeId, UIA_NamePropertyId, UIA_PROPERTY_ID, UIA_PaneControlTypeId,
        UIA_RadioButtonControlTypeId, UIA_SelectionItemIsSelectedPropertyId, UIA_TabControlTypeId,
        UIA_TabItemControlTypeId, UIA_TableControlTypeId, UIA_TextControlTypeId,
        UIA_ToggleToggleStatePropertyId, UIA_TreeControlTypeId, UIA_TreeItemControlTypeId,
        UIA_ValueValuePropertyId, UIA_WindowControlTypeId,
    },
};

use super::{
    error::{UiaError, UiaOperation},
    native::{
        UiaControlType, UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty,
        UiaRoleConstraint,
    },
    plan::UiaMatcherPlan,
};

/// 创建 Control View、角色与全部原生谓词的 AND condition。
pub(crate) fn build_match_condition(
    automation: &IUIAutomation,
    matcher: &UiaMatcherPlan,
) -> Result<IUIAutomationCondition, UiaError> {
    // ControlViewCondition 确保每次 matcher 都排除 raw provider fragment。
    // SAFETY: automation client 只由当前 UIA worker apartment 调用。
    let mut condition = unsafe { automation.ControlViewCondition() }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
    condition = and_condition(
        automation,
        condition,
        role_condition(automation, matcher.role)?,
    )?;
    for predicate in &matcher.pushdown {
        let predicate_condition = predicate_condition(automation, predicate)?;
        condition = and_condition(automation, condition, predicate_condition)?;
    }
    Ok(condition)
}

/// 物化对话框的复合约束或普通 ControlType 条件。
fn role_condition(
    automation: &IUIAutomation,
    role: UiaRoleConstraint,
) -> Result<IUIAutomationCondition, UiaError> {
    match role {
        UiaRoleConstraint::ControlType(control_type) => {
            control_type_condition(automation, control_type)
        }
        UiaRoleConstraint::Dialog => {
            let window = control_type_condition(automation, UiaControlType::Window)?;
            let dialog = property_condition(
                automation,
                UiaProperty::IsDialog,
                &UiaNativeValue::Boolean(true),
            )?;
            and_condition(automation, window, dialog)
        }
    }
}

/// 创建单个 ControlType PropertyCondition。
fn control_type_condition(
    automation: &IUIAutomation,
    control_type: UiaControlType,
) -> Result<IUIAutomationCondition, UiaError> {
    let value = VARIANT::from(control_type_id(control_type));
    // SAFETY: property id 与 VARIANT 类型来自封闭映射，VARIANT 在同步调用期间有效。
    unsafe { automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &value) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
}

/// 创建等值 condition，并在需要时包裹 NotCondition。
fn predicate_condition(
    automation: &IUIAutomation,
    predicate: &UiaNativePredicate,
) -> Result<IUIAutomationCondition, UiaError> {
    let (value, negate) = match &predicate.comparison {
        UiaNativeComparison::Equal(value) => (value, false),
        UiaNativeComparison::NotEqual(value) => (value, true),
    };
    let condition = property_condition(automation, predicate.property, value)?;
    if negate {
        // SAFETY: condition 与 automation client 属于同一个 UIA worker apartment。
        unsafe { automation.CreateNotCondition(&condition) }
            .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
    } else {
        Ok(condition)
    }
}

/// 将强类型 property/value 转换为 PropertyCondition。
fn property_condition(
    automation: &IUIAutomation,
    property: UiaProperty,
    value: &UiaNativeValue,
) -> Result<IUIAutomationCondition, UiaError> {
    let variant = native_variant(value);
    // SAFETY: property/value 组合已经由 compiler 证明，VARIANT 在同步调用期间有效。
    unsafe { automation.CreatePropertyCondition(property_id(property), &variant) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
}

/// 合并两个已经创建的原生 condition。
fn and_condition(
    automation: &IUIAutomation,
    left: IUIAutomationCondition,
    right: IUIAutomationCondition,
) -> Result<IUIAutomationCondition, UiaError> {
    // SAFETY: 两个 condition 与 automation client 均由同一个 worker apartment 创建。
    unsafe { automation.CreateAndCondition(&left, &right) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
}

/// 创建 Windows VARIANT，同时由 Rust 所有权管理 BSTR 生命周期。
fn native_variant(value: &UiaNativeValue) -> VARIANT {
    match value {
        UiaNativeValue::Text(value) => VARIANT::from(value.as_str()),
        UiaNativeValue::Boolean(value) => VARIANT::from(*value),
        UiaNativeValue::Integer(value) => VARIANT::from(*value),
    }
}

/// 返回 UIA property id；映射只存在于 executor native 边界。
pub(crate) const fn property_id(property: UiaProperty) -> UIA_PROPERTY_ID {
    match property {
        UiaProperty::Name => UIA_NamePropertyId,
        UiaProperty::AutomationId => UIA_AutomationIdPropertyId,
        UiaProperty::ClassName => UIA_ClassNamePropertyId,
        UiaProperty::Value => UIA_ValueValuePropertyId,
        UiaProperty::IsEnabled => UIA_IsEnabledPropertyId,
        UiaProperty::IsOffscreen => UIA_IsOffscreenPropertyId,
        UiaProperty::HasKeyboardFocus => UIA_HasKeyboardFocusPropertyId,
        UiaProperty::ToggleState => UIA_ToggleToggleStatePropertyId,
        UiaProperty::IsSelected => UIA_SelectionItemIsSelectedPropertyId,
        UiaProperty::IsDialog => UIA_IsDialogPropertyId,
    }
}

/// 返回 UIA ControlType 的原生整数值。
const fn control_type_id(control_type: UiaControlType) -> i32 {
    match control_type {
        UiaControlType::Window => UIA_WindowControlTypeId.0,
        UiaControlType::Pane => UIA_PaneControlTypeId.0,
        UiaControlType::Button => UIA_ButtonControlTypeId.0,
        UiaControlType::Edit => UIA_EditControlTypeId.0,
        UiaControlType::CheckBox => UIA_CheckBoxControlTypeId.0,
        UiaControlType::RadioButton => UIA_RadioButtonControlTypeId.0,
        UiaControlType::ComboBox => UIA_ComboBoxControlTypeId.0,
        UiaControlType::List => UIA_ListControlTypeId.0,
        UiaControlType::ListItem => UIA_ListItemControlTypeId.0,
        UiaControlType::Tree => UIA_TreeControlTypeId.0,
        UiaControlType::TreeItem => UIA_TreeItemControlTypeId.0,
        UiaControlType::Tab => UIA_TabControlTypeId.0,
        UiaControlType::TabItem => UIA_TabItemControlTypeId.0,
        UiaControlType::Menu => UIA_MenuControlTypeId.0,
        UiaControlType::MenuItem => UIA_MenuItemControlTypeId.0,
        UiaControlType::Hyperlink => UIA_HyperlinkControlTypeId.0,
        UiaControlType::Image => UIA_ImageControlTypeId.0,
        UiaControlType::Table => UIA_TableControlTypeId.0,
        UiaControlType::Document => UIA_DocumentControlTypeId.0,
        UiaControlType::Text => UIA_TextControlTypeId.0,
    }
}
