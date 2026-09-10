//! UIA 原生读取只发生在 MTA 上；不支持的 Pattern 保持属性缺失。
use crate::platform::failure;
use crate::{ElementSnapshot, WindowsError};
use argusflow_aql::{Attribute as A, Capabilities, Role, Value};
use std::collections::{BTreeMap, BTreeSet};
use windows::{
    Win32::{Foundation::E_NOINTERFACE, UI::Accessibility::*},
    core::Interface,
};

pub(super) fn capabilities() -> Capabilities {
    Capabilities::new(
        Role::ALL.iter().copied(),
        A::ALL.iter().copied().filter(|attribute| {
            !matches!(
                attribute,
                A::Confidence | A::DomId | A::TestId | A::DomClass | A::Tag
            )
        }),
    )
    .with_relations()
}
pub(super) fn role(
    element: &IUIAutomationElement,
    snapshot: &ElementSnapshot,
) -> Result<Role, WindowsError> {
    if matches!(snapshot.control_type, 50025 | 50029)
        && pattern::<IUIAutomationGridItemPattern>(element, UIA_GridItemPatternId)?.is_some()
    {
        return Ok(Role::Cell);
    }
    Ok(match snapshot.control_type {
        50000 => Role::Button,
        50002 => Role::CheckBox,
        50003 => Role::ComboBox,
        50004 => Role::TextBox,
        50005 => Role::Link,
        50006 => Role::Image,
        50007 => Role::ListItem,
        50008 => Role::List,
        50009 | 50010 => Role::Menu,
        50011 => Role::MenuItem,
        50013 => Role::Radio,
        50018 => Role::Tab,
        50019 => Role::TabItem,
        50020 => Role::Text,
        50023 => Role::Tree,
        50024 => Role::TreeItem,
        50028 | 50036 => Role::Table,
        50029 => Role::Row,
        50030 => Role::Document,
        50032 => {
            let modern: IUIAutomationElement9 = element
                .cast()
                .map_err(|e| failure("uia_dialog_interface", e))?;
            // SAFETY: 该接口由同一 worker MTA 中的原始元素获得。
            if unsafe { modern.CurrentIsDialog() }
                .map_err(|e| failure("uia_dialog", e))?
                .as_bool()
            {
                Role::Dialog
            } else {
                Role::Window
            }
        }
        50033 => Role::Pane,
        _ => Role::Element,
    })
}
fn pattern<P: Interface>(
    element: &IUIAutomationElement,
    id: UIA_PATTERN_ID,
) -> Result<Option<P>, WindowsError> {
    // SAFETY: Pattern 留在原有 MTA；只有明确的不支持错误可作为缺失。
    match unsafe { element.GetCurrentPatternAs::<P>(id) } {
        Ok(pattern) => Ok(Some(pattern)),
        Err(error)
            if error.code().is_ok()
                || error.code().0 as u32 == UIA_E_NOTSUPPORTED
                || error.code() == E_NOINTERFACE =>
        {
            Ok(None)
        }
        Err(error) => Err(failure("uia_pattern_read", error)),
    }
}
pub(super) fn read(
    element: &IUIAutomationElement,
    snapshot: &ElementSnapshot,
    requested: &BTreeSet<A>,
) -> Result<BTreeMap<A, Value>, WindowsError> {
    let mut values = BTreeMap::new();
    for attribute in requested
        .iter()
        .copied()
        .chain([A::Name, A::Enabled, A::Visible])
    {
        // SAFETY: 全部属性访问和临时 Pattern 都位于单个 MTA 请求。
        let value = unsafe {
            match attribute {
                A::Name => Some(Value::Text(snapshot.name.clone())),
                A::Key | A::AutomationId => Some(Value::Text(snapshot.automation_id.clone())),
                A::ClassName => Some(Value::Text(snapshot.class_name.clone())),
                A::Enabled => Some(Value::Boolean(snapshot.enabled)),
                A::Visible => Some(Value::Boolean(!snapshot.offscreen)),
                A::Focused => Some(Value::Boolean(
                    element
                        .CurrentHasKeyboardFocus()
                        .map_err(|e| failure("uia_focus_read", e))?
                        .as_bool(),
                )),
                A::AcceleratorKey => Some(Value::Text(
                    element
                        .CurrentAcceleratorKey()
                        .map_err(|e| failure("uia_accelerator", e))?
                        .to_string(),
                )),
                A::AccessKey => Some(Value::Text(
                    element
                        .CurrentAccessKey()
                        .map_err(|e| failure("uia_access_key", e))?
                        .to_string(),
                )),
                A::FrameworkId => Some(Value::Text(
                    element
                        .CurrentFrameworkId()
                        .map_err(|e| failure("uia_framework", e))?
                        .to_string(),
                )),
                A::Value => pattern::<IUIAutomationValuePattern>(element, UIA_ValuePatternId)?
                    .map(|p| p.CurrentValue().map(|s| Value::Text(s.to_string())))
                    .transpose()
                    .map_err(|e| failure("uia_value", e))?,
                A::Checked => {
                    match pattern::<IUIAutomationTogglePattern>(element, UIA_TogglePatternId)? {
                        Some(p) => match p
                            .CurrentToggleState()
                            .map_err(|e| failure("uia_checked", e))?
                        {
                            value if value == ToggleState_On => Some(Value::Boolean(true)),
                            value if value == ToggleState_Off => Some(Value::Boolean(false)),
                            _ => None,
                        },
                        None => None,
                    }
                }
                A::Selected => pattern::<IUIAutomationSelectionItemPattern>(
                    element,
                    UIA_SelectionItemPatternId,
                )?
                .map(|p| p.CurrentIsSelected().map(|b| Value::Boolean(b.as_bool())))
                .transpose()
                .map_err(|e| failure("uia_selected", e))?,
                A::Text if snapshot.control_type == 50020 => {
                    Some(Value::Text(snapshot.name.clone()))
                }
                A::Text => pattern::<IUIAutomationTextPattern>(element, UIA_TextPatternId)?
                    .map(|p| {
                        p.DocumentRange()
                            .and_then(|r| r.GetText(65_537))
                            .map(|s| Value::Text(s.to_string()))
                    })
                    .transpose()
                    .map_err(|e| failure("uia_text", e))?,
                _ => None,
            }
        };
        if let Some(value) = value {
            values.insert(attribute, value);
        }
    }
    Ok(values)
}
