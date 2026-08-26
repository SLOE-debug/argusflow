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
    native::{UiaControlType, UiaProperty, UiaRoleConstraint},
    plan::UiaMatcherPlan,
};

/// 仅通过数值型 ControlType 条件发现候选，字符串等值由 Rust 读取 Current property 复验。
pub(crate) fn build_discovery_condition(
    automation: &IUIAutomation,
    matcher: &UiaMatcherPlan,
) -> Result<IUIAutomationCondition, UiaError> {
    role_condition(automation, matcher.role)
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
        // Window ControlType 负责发现候选，IsDialog 在 Rust 强类型复验阶段检查。
        UiaRoleConstraint::Dialog => control_type_condition(automation, UiaControlType::Window),
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
