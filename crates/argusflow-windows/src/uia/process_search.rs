//! 通过选择性条件发现 UIA provider fragment，并强类型复验完整 matcher 与 PID。

use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement, TreeScope_Descendants,
};

use super::{
    budget::UiaBudgetTracker,
    cache::build_cache_request,
    condition::build_discovery_condition,
    current_match::matches_current,
    error::{UiaError, UiaOperation},
    plan::UiaMatcherPlan,
    property::matches_residual,
};

/// 从桌面根执行单个选择性条件，并在返回动作目标前复验完整 matcher 与当前 PID。
///
/// Notepad++ 等 provider 的菜单和对话框 fragment 可能独立于原始 HWND；这里按角色发现
/// 桌面候选，再由 Rust 读取 Current property 求值，并用 PID 建立应用会话硬边界。
pub(super) fn find_process_matches(
    automation: &IUIAutomation,
    process_id: u32,
    matcher: &UiaMatcherPlan,
    budget: &mut UiaBudgetTracker,
) -> Result<Vec<IUIAutomationElement>, UiaError> {
    let process_id =
        i32::try_from(process_id).map_err(|_| UiaError::InvalidProcessId { process_id })?;
    let condition = build_discovery_condition(automation, matcher)?;
    // 只有 residual 谓词需要 cached property；纯原生查询不创建缓存请求。
    let cache = if matcher.residual.is_empty() {
        None
    } else {
        Some(build_cache_request(automation, &matcher.cache)?)
    };
    budget.check_deadline()?;
    // SAFETY: automation client 与返回的桌面根元素只在当前 UIA worker apartment 使用。
    let desktop = unsafe { automation.GetRootElement() }
        .map_err(|source| UiaError::from_native(UiaOperation::GetDesktopRoot, source))?;
    // SAFETY: 选择性条件允许 provider 物化按需 fragment，完整 matcher 随后在 Rust 复验。
    let candidates = unsafe { desktop.FindAll(TreeScope_Descendants, &condition) }
        .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
    let candidate_count = element_array_length(&candidates)?;
    budget.observe_traversal_nodes(candidate_count)?;
    let mut matches = Vec::with_capacity(candidate_count);
    for candidate_index in 0..candidate_count {
        budget.check_deadline()?;
        // SAFETY: candidate_index 来自已验证的数组长度，array 留在当前 apartment。
        let element = unsafe { candidates.GetElement(candidate_index as i32) }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        // SAFETY: element 留在当前 UIA worker apartment，仅同步复验候选所属进程。
        let candidate_process_id = unsafe { element.CurrentProcessId() }
            .map_err(|source| UiaError::from_native(UiaOperation::ReadProperty, source))?;
        if candidate_process_id != process_id {
            continue;
        }
        if !matches_current(&element, matcher)? {
            continue;
        }
        let element = update_cache(element, cache.as_ref())?;
        if matches_residual(&element, &matcher.residual)? {
            matches.push(element);
        }
    }
    Ok(matches)
}

/// 把 provider 返回的 i32 数组长度校验并收窄为本地索引类型。
fn element_array_length(
    elements: &windows::Win32::UI::Accessibility::IUIAutomationElementArray,
) -> Result<usize, UiaError> {
    // SAFETY: element array 没有离开创建它的 UIA worker apartment。
    let length = unsafe { elements.Length() }
        .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
    usize::try_from(length).map_err(|_| UiaError::InvalidCandidateCount { count: length })
}

/// 为含 residual 的候选补充精确属性缓存；原生 matcher 直接保留当前元素。
fn update_cache(
    element: IUIAutomationElement,
    cache: Option<&IUIAutomationCacheRequest>,
) -> Result<IUIAutomationElement, UiaError> {
    let Some(cache) = cache else {
        return Ok(element);
    };
    // SAFETY: element 与 cache 同属当前 UIA worker apartment，cache 仅请求 Element scope。
    unsafe { element.BuildUpdatedCache(cache) }
        .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))
}
