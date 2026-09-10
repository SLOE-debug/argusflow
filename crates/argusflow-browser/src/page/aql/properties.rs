//! 浏览器提供可访问名称和角色，DOM 提供当前表单和布局属性。
use super::model::{AxNode, DomFacts};
use argusflow_aql::{Attribute as A, Role, Value};
use std::collections::BTreeMap;

pub(super) fn role(node: Option<&AxNode>) -> Role {
    match node
        .and_then(|n| n.role.as_ref())
        .and_then(|v| v.value.as_str())
        .unwrap_or("")
    {
        "button" => Role::Button,
        "textbox" | "searchbox" => Role::TextBox,
        "checkbox" => Role::CheckBox,
        "radio" => Role::Radio,
        "combobox" => Role::ComboBox,
        "list" => Role::List,
        "listitem" => Role::ListItem,
        "tree" => Role::Tree,
        "treeitem" => Role::TreeItem,
        "tablist" => Role::Tab,
        "tab" => Role::TabItem,
        "menu" | "menubar" => Role::Menu,
        "menuitem" | "menuitemcheckbox" | "menuitemradio" => Role::MenuItem,
        "link" => Role::Link,
        "image" | "img" => Role::Image,
        "table" | "grid" => Role::Table,
        "row" => Role::Row,
        "cell" | "gridcell" | "columnheader" | "rowheader" => Role::Cell,
        "RootWebArea" | "WebArea" | "document" => Role::Document,
        "dialog" | "alertdialog" => Role::Dialog,
        "StaticText" | "InlineTextBox" => Role::Text,
        "generic" | "group" | "region" => Role::Pane,
        _ => Role::Element,
    }
}
pub(super) fn attributes(facts: &DomFacts, ax: Option<&AxNode>, tag: &str) -> BTreeMap<A, Value> {
    let mut values = BTreeMap::new();
    if let Some(name) = ax
        .and_then(|n| n.name.as_ref())
        .and_then(|v| v.value.as_str())
    {
        values.insert(A::Name, Value::Text(name.into()));
    }
    for (attribute, value) in [
        (A::Text, facts.text.as_ref()),
        (A::Value, facts.value.as_ref()),
    ] {
        if let Some(value) = value {
            values.insert(attribute, Value::Text(value.clone()));
        }
    }
    for (attribute, value) in [
        (A::Enabled, facts.enabled),
        (A::Visible, Some(facts.visible)),
        (A::Focused, Some(facts.focused)),
        (A::Checked, facts.checked),
        (A::Selected, facts.selected),
    ] {
        if let Some(value) = value {
            values.insert(attribute, Value::Boolean(value));
        }
    }
    for (attribute, key) in [
        (A::Key, "id"),
        (A::DomId, "id"),
        (A::TestId, "data-testid"),
        (A::DomClass, "class"),
    ] {
        if let Some(value) = facts.attributes.get(key) {
            values.insert(attribute, Value::Text(value.clone()));
        }
    }
    if !tag.is_empty() {
        values.insert(A::Tag, Value::Text(tag.into()));
    }
    values
}
