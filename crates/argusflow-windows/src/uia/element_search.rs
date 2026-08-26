//! UIA 原生条件查询与 CacheRequest 批量物化。

use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, IUIAutomationCondition, IUIAutomationElement,
    TreeScope, TreeScope_Children, TreeScope_Descendants, TreeScope_Element,
};

use super::{
    budget::UiaBudgetTracker,
    element_identity::runtime_id,
    error::{UiaError, UiaOperation},
    native::UiaResidualPredicate,
    plan::UiaResultLimit,
    property::matches_residual,
    target_selection::{ResolvedElement, ResolvedElementSet},
};

/// 用原生条件查询候选，并在同一次 provider 调用中缓存 residual 属性。
pub(super) fn find_cached_matches(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    scope: TreeScope,
    condition: &IUIAutomationCondition,
    cache: &IUIAutomationCacheRequest,
    residual: &[UiaResidualPredicate],
    result_limit: UiaResultLimit,
    budget: &mut UiaBudgetTracker,
) -> Result<Vec<ResolvedElement>, UiaError> {
    budget.check_deadline()?;

    // 没有本地 residual 时，First 可以直接让 provider 停在第一个原生匹配项。
    if can_find_first(result_limit, residual.is_empty()) {
        // SAFETY: root、condition 与 cache 均属于当前 UIA worker apartment。
        let element =
            optional_element(unsafe { root.FindFirstBuildCache(scope, condition, cache) })?;
        if element.is_some() {
            budget.observe_traversal_nodes(1)?;
        }
        let Some(element) = element else {
            return Ok(Vec::new());
        };
        return Ok(vec![ResolvedElement {
            runtime_id: runtime_id(&element)?,
            element,
        }]);
    }
    if let Some(limit) = bounded_tree_limit(scope, result_limit) {
        return walk_cached_matches(
            automation, condition, root, scope, cache, residual, limit, budget,
        );
    }

    // SAFETY: root、condition 与 cache 均属于当前 UIA worker apartment。
    let candidates = unsafe { root.FindAllBuildCache(scope, condition, cache) }
        .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
    let candidate_count = element_array_length(&candidates)?;
    budget.observe_traversal_nodes(candidate_count)?;
    let capacity = result_limit
        .maximum()
        .map(|limit| limit.min(candidate_count))
        .unwrap_or(candidate_count);
    let mut matches = ResolvedElementSet::with_capacity(capacity);
    for candidate_index in 0..candidate_count {
        budget.check_deadline()?;
        // SAFETY: candidate_index 来自已验证的数组长度，array 留在当前 apartment。
        let element = unsafe { candidates.GetElement(candidate_index as i32) }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        if matches_residual(&element, residual)? {
            matches.insert(ResolvedElement {
                runtime_id: runtime_id(&element)?,
                element,
            });
            if result_limit.is_reached(matches.len()) {
                break;
            }
        }
    }
    Ok(matches.into_vec())
}

/// 只有不依赖本地 residual 的 First 查询可以直接使用 provider 的单结果 API。
const fn can_find_first(result_limit: UiaResultLimit, has_residual: bool) -> bool {
    matches!(result_limit, UiaResultLimit::AtMost(count) if count.get() == 1) && !has_residual
}

/// Children/Descendants 查询可用 Raw View 在达到 First/Nth 上限时停止 provider 导航。
fn bounded_tree_limit(
    scope: TreeScope,
    result_limit: UiaResultLimit,
) -> Option<std::num::NonZeroUsize> {
    if scope == TreeScope_Children || scope == TreeScope_Descendants {
        match result_limit {
            UiaResultLimit::All => None,
            UiaResultLimit::AtMost(limit) => Some(limit),
        }
    } else {
        None
    }
}

/// 使用 Raw View 深度优先遍历，并在本地应用与 Find* 相同的完整原生条件。
///
/// `CreateTreeWalker(condition)` 会创建重排层级的过滤视图，不能同时保持原始
/// `TreeScope_Children` 与 `TreeScope_Descendants` 边界；因此有界 First/Nth 在 Raw View
/// 中导航，并对每个候选复用冻结的原生 condition。
fn walk_cached_matches(
    automation: &IUIAutomation,
    condition: &IUIAutomationCondition,
    root: &IUIAutomationElement,
    scope: TreeScope,
    cache: &IUIAutomationCacheRequest,
    residual: &[UiaResidualPredicate],
    limit: std::num::NonZeroUsize,
    budget: &mut UiaBudgetTracker,
) -> Result<Vec<ResolvedElement>, UiaError> {
    // SAFETY: RawViewCondition 与 automation 同属当前 UIA worker apartment。
    let raw_view = unsafe { automation.RawViewCondition() }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
    // SAFETY: raw_view 与 automation 同属当前 UIA worker apartment。
    let walker = unsafe { automation.CreateTreeWalker(&raw_view) }
        .map_err(|source| UiaError::from_native(UiaOperation::CreateCondition, source))?;
    // SAFETY: walker 与 root 均属于当前 worker apartment；导航元素随后通过带缓存的
    // FindFirstBuildCache 物化，避免在导航阶段重复预取 residual 属性。
    let first = optional_navigation_element(unsafe { walker.GetFirstChildElement(root) })?;
    let mut pending = Vec::new();
    if let Some(first) = first {
        pending.push(first);
    }
    // Nth 可远大于实际预算，预分配只采用小容量提示，避免恶意索引触发大块内存申请。
    let mut matches = ResolvedElementSet::with_capacity(limit.get().min(32));
    while let Some(element) = pending.pop() {
        budget.observe_traversal_nodes(1)?;
        budget.check_deadline()?;
        // SAFETY: element、condition 与 automation 均留在当前 UIA worker apartment。
        let native_match = optional_element(unsafe {
            element.FindFirstBuildCache(TreeScope_Element, condition, cache)
        })?;
        if let Some(native_match) = native_match
            && matches_residual(&native_match, residual)?
        {
            matches.insert(ResolvedElement {
                runtime_id: runtime_id(&native_match)?,
                element: native_match,
            });
            if matches.len() >= limit.get() {
                break;
            }
        }

        // 先压入 sibling，再压入 child，使 LIFO 栈保持深度优先的 provider 顺序。
        let sibling =
            optional_navigation_element(unsafe { walker.GetNextSiblingElement(&element) })?;
        if let Some(sibling) = sibling {
            pending.push(sibling);
        }
        if scope == TreeScope_Descendants {
            let child =
                optional_navigation_element(unsafe { walker.GetFirstChildElement(&element) })?;
            if let Some(child) = child {
                pending.push(child);
            }
        }
    }
    Ok(matches.into_vec())
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

/// 把 FindFirst 的 S_OK + null 结束标记与真正 provider 错误分开。
fn optional_element(
    result: windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, UiaError> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(source) if source.code().0 == 0 => Ok(None),
        Err(source) => Err(UiaError::from_native(UiaOperation::FindFirst, source)),
    }
}

/// 把 TreeWalker 的 S_OK + null 结束标记与真正 provider 错误分开。
fn optional_navigation_element(
    result: windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, UiaError> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(source) if source.code().0 == 0 => Ok(None),
        Err(source) => Err(UiaError::from_native(UiaOperation::NavigateTree, source)),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use windows::Win32::UI::Accessibility::{TreeScope_Children, TreeScope_Descendants};

    use super::{bounded_tree_limit, can_find_first};
    use crate::uia::plan::UiaResultLimit;

    /// 原生 First 可以在 provider 端停止；residual 和 Nth 必须继续检查候选顺序。
    #[test]
    fn find_first_requires_single_native_result() {
        let two_results = NonZeroUsize::new(2).expect("two is non-zero");

        assert!(can_find_first(UiaResultLimit::first(), false));
        assert!(!can_find_first(UiaResultLimit::first(), true));
        assert!(!can_find_first(UiaResultLimit::at_most(two_results), false));
        assert!(!can_find_first(UiaResultLimit::All, false));
    }

    /// 有界 Raw View 同时覆盖直接子级和后代，并保留各自的原生 scope。
    #[test]
    fn bounded_walker_preserves_supported_tree_scopes() {
        let three_results = NonZeroUsize::new(3).expect("three is non-zero");

        assert_eq!(
            bounded_tree_limit(
                TreeScope_Descendants,
                UiaResultLimit::at_most(three_results),
            ),
            Some(three_results)
        );
        assert_eq!(
            bounded_tree_limit(TreeScope_Children, UiaResultLimit::at_most(three_results),),
            Some(three_results)
        );
        assert_eq!(
            bounded_tree_limit(TreeScope_Descendants, UiaResultLimit::All),
            None
        );
    }
}
