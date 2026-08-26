//! 通过 ProcessId 与完整原生 matcher 发现 UIA provider fragment。

use windows::Win32::UI::Accessibility::{IUIAutomation, TreeScope_Descendants};

use super::{
    budget::UiaBudgetTracker,
    cache::build_cache_request,
    condition::build_process_match_condition,
    element_search::find_cached_matches,
    error::{UiaError, UiaOperation},
    plan::{UiaMatcherPlan, UiaResultLimit},
    target_selection::ResolvedElement,
};

/// 从桌面根执行带 ProcessId 硬边界的完整原生条件。
///
/// Notepad++ 等 provider 的菜单和对话框 fragment 可能独立于原始 HWND；这里按角色发现
/// 桌面候选，但候选生成已由 ProcessId、Control View、角色和等值谓词共同收窄。
pub(super) fn find_process_matches(
    automation: &IUIAutomation,
    process_id: u32,
    matcher: &UiaMatcherPlan,
    result_limit: UiaResultLimit,
    budget: &mut UiaBudgetTracker,
) -> Result<Vec<ResolvedElement>, UiaError> {
    let process_id =
        i32::try_from(process_id).map_err(|_| UiaError::InvalidProcessId { process_id })?;
    let condition = build_process_match_condition(automation, process_id, matcher)?;
    let cache = build_cache_request(automation, &matcher.cache)?;
    budget.check_deadline()?;
    // SAFETY: automation client 与返回的桌面根元素只在当前 UIA worker apartment 使用。
    let desktop = unsafe { automation.GetRootElement() }
        .map_err(|source| UiaError::from_native(UiaOperation::GetDesktopRoot, source))?;
    find_cached_matches(
        automation,
        &desktop,
        TreeScope_Descendants,
        &condition,
        &cache,
        &matcher.residual,
        result_limit,
        budget,
    )
}
