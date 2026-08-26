//! `UiaMatcherPlan` 到原生 `IUIAutomationCondition` 的物化。

use windows::Win32::{
    System::Variant::VARIANT,
    UI::Accessibility::{
        IUIAutomation, IUIAutomationCondition, UIA_AcceleratorKeyPropertyId,
        UIA_AccessKeyPropertyId, UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId,
        UIA_CheckBoxControlTypeId, UIA_ClassNamePropertyId, UIA_ComboBoxControlTypeId,
        UIA_ControlTypePropertyId, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
        UIA_FrameworkIdPropertyId, UIA_HasKeyboardFocusPropertyId, UIA_HyperlinkControlTypeId,
        UIA_ImageControlTypeId, UIA_IsDialogPropertyId, UIA_IsEnabledPropertyId,
        UIA_IsOffscreenPropertyId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
        UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_NamePropertyId, UIA_PROPERTY_ID,
        UIA_PaneControlTypeId, UIA_ProcessIdPropertyId, UIA_RadioButtonControlTypeId,
        UIA_SelectionItemIsSelectedPropertyId, UIA_TabControlTypeId, UIA_TabItemControlTypeId,
        UIA_TableControlTypeId, UIA_TextControlTypeId, UIA_ToggleToggleStatePropertyId,
        UIA_TreeControlTypeId, UIA_TreeItemControlTypeId, UIA_ValueValuePropertyId,
        UIA_WindowControlTypeId,
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

/// 创建完整 matcher 的 Control View 原生条件。
pub(crate) fn build_match_condition(
    automation: &IUIAutomation,
    matcher: &UiaMatcherPlan,
) -> Result<IUIAutomationCondition, UiaError> {
    let control_view = unsafe { automation.ControlViewCondition() }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
    let role = role_condition(automation, matcher.role)?;
    let mut condition = and_condition(automation, &control_view, &role)?;
    for predicate in &matcher.pushdown {
        let predicate = predicate_condition(automation, predicate)?;
        condition = and_condition(automation, &condition, &predicate)?;
    }
    Ok(condition)
}

/// 创建带冻结 ProcessId 硬边界的完整 matcher 原生条件。
pub(crate) fn build_process_match_condition(
    automation: &IUIAutomation,
    process_id: i32,
    matcher: &UiaMatcherPlan,
) -> Result<IUIAutomationCondition, UiaError> {
    let matcher = build_match_condition(automation, matcher)?;
    let process_value = VARIANT::from(process_id);
    // SAFETY: ProcessId property 接受 VT_I4，VARIANT 在同步调用期间有效。
    let process =
        unsafe { automation.CreatePropertyCondition(UIA_ProcessIdPropertyId, &process_value) }
            .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
    and_condition(automation, &process, &matcher)
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
            let is_dialog = property_condition(
                automation,
                UiaProperty::IsDialog,
                &UiaNativeValue::Boolean(true),
            )?;
            and_condition(automation, &window, &is_dialog)
        }
    }
}

/// 把强类型原生谓词物化为 PropertyCondition 或 NotCondition。
fn predicate_condition(
    automation: &IUIAutomation,
    predicate: &UiaNativePredicate,
) -> Result<IUIAutomationCondition, UiaError> {
    match &predicate.comparison {
        UiaNativeComparison::Equal(value) => {
            property_condition(automation, predicate.property, value)
        }
        UiaNativeComparison::NotEqual(value) => {
            let equal = property_condition(automation, predicate.property, value)?;
            // SAFETY: equal condition 与 automation 由同一 worker apartment 创建。
            unsafe { automation.CreateNotCondition(&equal) }
                .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
        }
    }
}

/// 创建一个属性等值条件，值类型由 compiler 的原生 IR 保证正确。
fn property_condition(
    automation: &IUIAutomation,
    property: UiaProperty,
    value: &UiaNativeValue,
) -> Result<IUIAutomationCondition, UiaError> {
    let value = match value {
        UiaNativeValue::Text(value) => VARIANT::from(value.as_str()),
        UiaNativeValue::Boolean(value) => VARIANT::from(*value),
        UiaNativeValue::Integer(value) => VARIANT::from(*value),
    };
    // SAFETY: property id 和 VARIANT 来自 compiler 证明过的封闭强类型映射。
    unsafe { automation.CreatePropertyCondition(property_id(property), &value) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
}

/// 合并两个同 apartment 原生条件。
fn and_condition(
    automation: &IUIAutomation,
    left: &IUIAutomationCondition,
    right: &IUIAutomationCondition,
) -> Result<IUIAutomationCondition, UiaError> {
    // SAFETY: 两个 condition 与 automation 均由当前 UIA worker apartment 创建。
    unsafe { automation.CreateAndCondition(left, right) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))
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

/// 返回 UIA property id；映射只存在于 executor native 边界。
pub(crate) const fn property_id(property: UiaProperty) -> UIA_PROPERTY_ID {
    match property {
        UiaProperty::Name => UIA_NamePropertyId,
        UiaProperty::AutomationId => UIA_AutomationIdPropertyId,
        UiaProperty::ClassName => UIA_ClassNamePropertyId,
        UiaProperty::AcceleratorKey => UIA_AcceleratorKeyPropertyId,
        UiaProperty::AccessKey => UIA_AccessKeyPropertyId,
        UiaProperty::FrameworkId => UIA_FrameworkIdPropertyId,
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
pub(crate) const fn control_type_id(control_type: UiaControlType) -> i32 {
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
