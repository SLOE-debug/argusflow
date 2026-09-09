//! 有界 UIA 遍历和类型化谓词求值。
use super::element::{ElementSnapshot, Predicate, Query, SearchScope};
use crate::WindowsError as Failure;
use crate::{
    UiaConfig,
    platform::{failure, hwnd},
};
use argusflow_core::{FailureKind, Operation};
use std::collections::VecDeque;
use windows::Win32::UI::Accessibility::{
    IUIAutomation2, IUIAutomationElement, IUIAutomationTreeWalker,
};

pub(crate) fn snapshot(element: &IUIAutomationElement) -> Result<ElementSnapshot, Failure> {
    // SAFETY: 所有读取在同一个 MTA apartment 内完成，结果复制为普通 Rust 数据。
    unsafe {
        let bounds = element
            .CurrentBoundingRectangle()
            .map_err(|e| failure("bounds", e))?;
        Ok(ElementSnapshot {
            name: element
                .CurrentName()
                .map_err(|e| failure("name", e))?
                .to_string(),
            automation_id: element
                .CurrentAutomationId()
                .map_err(|e| failure("automation_id", e))?
                .to_string(),
            class_name: element
                .CurrentClassName()
                .map_err(|e| failure("class_name", e))?
                .to_string(),
            control_type: element
                .CurrentControlType()
                .map_err(|e| failure("control_type", e))?
                .0,
            enabled: element
                .CurrentIsEnabled()
                .map_err(|e| failure("enabled", e))?
                .as_bool(),
            offscreen: element
                .CurrentIsOffscreen()
                .map_err(|e| failure("offscreen", e))?
                .as_bool(),
            bounds: [bounds.left, bounds.top, bounds.right, bounds.bottom],
        })
    }
}

pub(crate) fn validate_predicate(
    predicate: &Predicate,
    depth: usize,
    count: &mut usize,
) -> Result<(), Failure> {
    *count += 1;
    if depth > 16 || *count > 128 {
        return Err(limit("查询条件过深或过多"));
    }
    match predicate {
        Predicate::All(predicates) | Predicate::OneOf(predicates) => {
            if predicates.is_empty() {
                return Err(Failure::new(
                    FailureKind::InvalidInput,
                    "query",
                    "条件组合不能为空",
                ));
            }
            for predicate in predicates {
                validate_predicate(predicate, depth + 1, count)?;
            }
        }
        Predicate::Not(predicate) => validate_predicate(predicate, depth + 1, count)?,
        Predicate::AutomationId(value) | Predicate::Name(value) | Predicate::ClassName(value)
            if value.len() > 16_384 =>
        {
            return Err(limit("查询字符串超过限制"));
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn matches(predicate: &Predicate, value: &ElementSnapshot) -> bool {
    match predicate {
        Predicate::Any => true,
        Predicate::AutomationId(expected) => expected == &value.automation_id,
        Predicate::Name(expected) => expected == &value.name,
        Predicate::ClassName(expected) => expected == &value.class_name,
        Predicate::ControlType(expected) => *expected as i32 == value.control_type,
        Predicate::All(predicates) => predicates.iter().all(|predicate| matches(predicate, value)),
        Predicate::OneOf(predicates) => {
            predicates.iter().any(|predicate| matches(predicate, value))
        }
        Predicate::Not(predicate) => !matches(predicate, value),
    }
}

/// windows-core 0.62 的 Type::from_abi 把成功返回的 null COM 指针转成 Error::empty (S_OK)。
/// 仅导航 API 允许此表示“没有节点”；真正失败的 HRESULT 必须传播。
fn optional_navigation(
    result: windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, Failure> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(error) if error.code().is_ok() => Ok(None),
        Err(error) => Err(failure("tree_navigation", error)),
    }
}

pub(crate) fn find(
    automation: &IUIAutomation2,
    query: &Query,
    operation: &Operation,
    config: &UiaConfig,
    unique: bool,
) -> Result<Vec<IUIAutomationElement>, Failure> {
    validate_predicate(&query.predicate, 0, &mut 0)?;
    query.window.validate()?;
    operation.check("query_root")?;
    // SAFETY: automation 和产生的元素均留在 worker MTA 线程。
    let root = unsafe { automation.ElementFromHandle(hwnd(query.window.handle())) }
        .map_err(|e| failure("query_root", e))?;
    // SAFETY: walker 与 automation 使用相同 apartment。
    let walker = unsafe { automation.RawViewWalker() }.map_err(|e| failure("tree_walker", e))?;
    let mut pending = VecDeque::from([(root, 0usize)]);
    let mut visited = 0;
    let mut result = Vec::new();
    while let Some((element, depth)) = pending.pop_front() {
        operation.check("query_traversal")?;
        visited += 1;
        if visited > config.max_nodes {
            return Err(limit("UIA 节点访问数量超限"));
        }
        let include = match query.scope {
            SearchScope::Root => depth == 0,
            SearchScope::Children => depth == 1,
            SearchScope::Descendants => depth > 0,
        };
        if include && matches(&query.predicate, &snapshot(&element)?) {
            result.push(element.clone());
            if unique && result.len() == 2 {
                return Err(Failure::new(
                    FailureKind::Ambiguous,
                    "query",
                    "唯一查询匹配到多个元素",
                ));
            }
            if result.len() > config.max_results {
                return Err(limit("UIA 查询结果数量超限"));
            }
        }
        let descend = match query.scope {
            SearchScope::Root => false,
            SearchScope::Children => depth == 0,
            SearchScope::Descendants => true,
        };
        if descend {
            push_children(
                &walker,
                &element,
                depth,
                &mut pending,
                visited,
                operation,
                config,
            )?;
        }
    }
    if unique && result.is_empty() {
        return Err(Failure::new(
            FailureKind::NotFound,
            "query",
            "没有匹配的 UIA 元素",
        ));
    }
    query.window.validate()?;
    operation.check("query_complete")?;
    Ok(result)
}

fn push_children(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    depth: usize,
    pending: &mut VecDeque<(IUIAutomationElement, usize)>,
    visited: usize,
    operation: &Operation,
    config: &UiaConfig,
) -> Result<(), Failure> {
    // SAFETY: COM 对象仅用于当前 apartment 的同步调用。
    let mut child = optional_navigation(unsafe { walker.GetFirstChildElement(element) })?;
    if child.is_some() && depth >= config.max_depth {
        return Err(limit("UIA 遍历深度超限"));
    }
    while let Some(element) = child {
        operation.check("query_children")?;
        if pending.len() + visited >= config.max_nodes {
            return Err(limit("UIA 遍历队列超限"));
        }
        // SAFETY: 同一 walker 的 sibling 导航。
        child = optional_navigation(unsafe { walker.GetNextSiblingElement(&element) })?;
        pending.push_back((element, depth + 1));
    }
    Ok(())
}

pub(crate) fn validate_membership(
    automation: &IUIAutomation2,
    element: &IUIAutomationElement,
    window: &crate::WindowIdentity,
    operation: &Operation,
    max_depth: usize,
) -> Result<(), Failure> {
    window.validate()?;
    // SAFETY: 全部对象在当前 MTA apartment 中使用，不向调用方泄漏 COM。
    unsafe {
        let root = automation
            .ElementFromHandle(hwnd(window.handle()))
            .map_err(|e| failure("element_root", e))?;
        let walker = automation
            .RawViewWalker()
            .map_err(|e| failure("element_walker", e))?;
        let mut current = Some(element.clone());
        for _ in 0..=max_depth {
            operation.check("element_identity")?;
            let Some(node) = current else { break };
            if automation
                .CompareElements(&root, &node)
                .map_err(|e| failure("element_identity", e))?
                .as_bool()
            {
                return Ok(());
            }
            current = optional_navigation(walker.GetParentElement(&node))?;
        }
    }
    Err(Failure::new(
        FailureKind::StaleHandle,
        "element_identity",
        "元素不再属于原窗口",
    ))
}

fn limit(message: &str) -> Failure {
    Failure::new(FailureKind::ResourceLimit, "query", message)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/uia/query.rs"]
mod tests;
