//! 一次有界先序遍历收集快照，仅给最终命中创建元素租约。
use super::super::query::{optional_navigation, snapshot};
use super::properties;
use crate::platform::{failure, hwnd};
use crate::{ElementSnapshot, UiaConfig, WindowIdentity, WindowsError};
use argusflow_aql::{Attribute, BoundQuery, Node, QueryTree, Value, evaluate};
use argusflow_core::Operation;
use std::collections::BTreeMap;
use windows::Win32::UI::Accessibility::{IUIAutomation2, IUIAutomationElement};

#[derive(Clone)]
pub(crate) struct Found {
    pub(crate) element: IUIAutomationElement,
    pub(crate) snapshot: ElementSnapshot,
    pub(crate) attributes: BTreeMap<Attribute, Value>,
}
pub(crate) fn find(
    automation: &IUIAutomation2,
    window: &WindowIdentity,
    query: &BoundQuery,
    operation: &Operation,
    config: &UiaConfig,
) -> Result<Vec<Found>, WindowsError> {
    properties::capabilities().check(query)?;
    window.validate()?;
    operation.check("uia_aql_root")?;
    // SAFETY: automation、walker 与所有遍历得到的 COM 对象均属于当前 MTA。
    let root = unsafe { automation.ElementFromHandle(hwnd(window.handle())) }
        .map_err(|e| failure("uia_aql_root", e))?;
    let walker = unsafe { automation.RawViewWalker() }.map_err(|e| failure("uia_aql_walker", e))?;
    let mut tree = QueryTree::new(config.max_nodes, config.max_depth, config.max_results)?;
    let attributes = query.attributes();
    let mut pending = vec![(root, None)];
    while let Some((element, parent)) = pending.pop() {
        operation.check("uia_aql_traverse")?;
        let snapshot = snapshot(&element)?;
        let role = properties::role(&element, &snapshot)?;
        let values = properties::read(&element, &snapshot, &attributes)?;
        let index = tree.push(Node::element(
            parent,
            role,
            values.clone(),
            Found {
                element: element.clone(),
                snapshot,
                attributes: values,
            },
        ))?;
        let mut children = Vec::new();
        // SAFETY: walker 的导航与元素读取使用同一 apartment。
        let mut child = optional_navigation(unsafe { walker.GetFirstChildElement(&element) })?;
        while let Some(element) = child {
            operation.check("uia_aql_children")?;
            if tree.len() + pending.len() + children.len() >= config.max_nodes {
                return Err(WindowsError::new(
                    argusflow_core::FailureKind::ResourceLimit,
                    "uia_aql",
                    "UIA 查询节点预算耗尽",
                ));
            }
            child = optional_navigation(unsafe { walker.GetNextSiblingElement(&element) })?;
            children.push(element);
        }
        pending.extend(
            children
                .into_iter()
                .rev()
                .map(|element| (element, Some(index))),
        );
    }
    let indices = evaluate(query, &tree, operation)?;
    window.validate()?;
    operation.check("uia_aql_complete")?;
    Ok(indices
        .into_iter()
        .filter_map(|index| tree.node(index).and_then(|node| node.target()).cloned())
        .collect())
}
