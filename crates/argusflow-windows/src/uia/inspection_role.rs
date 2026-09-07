//! 原生 UIA 控件类型到 AQL 角色的只读映射。

use argusflow_core::ElementRole;
use windows::Win32::UI::Accessibility::*;

/// 未知类型不猜成 Pane，交给 resolver 判定是否需要视觉回退。
#[allow(non_upper_case_globals)] // Win32 SDK 原名明确表示匹配常量，不重新定义裸数字协议。
pub(super) fn role(control_type: UIA_CONTROLTYPE_ID) -> Option<ElementRole> {
    Some(match control_type {
        UIA_WindowControlTypeId => ElementRole::Window,
        UIA_PaneControlTypeId => ElementRole::Pane,
        UIA_ButtonControlTypeId => ElementRole::Button,
        UIA_EditControlTypeId => ElementRole::TextBox,
        UIA_CheckBoxControlTypeId => ElementRole::CheckBox,
        UIA_RadioButtonControlTypeId => ElementRole::Radio,
        UIA_ComboBoxControlTypeId => ElementRole::ComboBox,
        UIA_ListControlTypeId => ElementRole::List,
        UIA_ListItemControlTypeId => ElementRole::ListItem,
        UIA_TreeControlTypeId => ElementRole::Tree,
        UIA_TreeItemControlTypeId => ElementRole::TreeItem,
        UIA_TabControlTypeId => ElementRole::Tab,
        UIA_TabItemControlTypeId => ElementRole::TabItem,
        UIA_MenuControlTypeId => ElementRole::Menu,
        UIA_MenuItemControlTypeId => ElementRole::MenuItem,
        UIA_HyperlinkControlTypeId => ElementRole::Link,
        UIA_ImageControlTypeId => ElementRole::Image,
        UIA_TableControlTypeId => ElementRole::Table,
        UIA_DocumentControlTypeId => ElementRole::Document,
        UIA_TextControlTypeId => ElementRole::Text,
        _ => return None,
    })
}
