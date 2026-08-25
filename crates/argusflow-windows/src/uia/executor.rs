//! HWND scoped UIA query algebra 执行、唯一目标解析与有限 stale 重试。

use std::ffi::c_void;

use argusflow_core::{ActionOutcome, AutomationError, BackendKind};
use windows::Win32::{
    Foundation::HWND,
    System::{
        Com::SAFEARRAY,
        Ole::{
            SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
            SafeArrayGetUBound,
        },
    },
    UI::{
        Accessibility::{
            IUIAutomation, IUIAutomationElement, IUIAutomationElementArray, TreeScope,
            TreeScope_Children, TreeScope_Descendants, TreeScope_Subtree,
        },
        WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
};

use super::{
    action::execute_action,
    budget::{UiaBudgetTracker, UiaExecutionBudget},
    cache::build_cache_request,
    condition::build_match_condition,
    error::{UiaError, UiaOperation},
    plan::{UiaMatcherPlan, UiaPlanExpr},
    property::matches_residual,
    runtime::{PreparedWindowTarget, UiaExecuteRequest},
};

/// UIA worker 线程内同步使用的查询执行器。
pub(crate) struct UiaExecutor<'automation> {
    /// 只在当前 COM apartment 创建和调用的 automation client。
    automation: &'automation IUIAutomation,
}

impl<'automation> UiaExecutor<'automation> {
    /// 绑定 worker 线程拥有的 UIA client。
    pub(crate) const fn new(automation: &'automation IUIAutomation) -> Self {
        Self { automation }
    }

    /// 用冻结的 HWND、查询计划和动作计划执行一次完整请求。
    pub(crate) fn execute(
        &self,
        request: UiaExecuteRequest,
        budget: UiaExecutionBudget,
    ) -> Result<ActionOutcome, AutomationError> {
        let mut budget = UiaBudgetTracker::new(budget);
        // root、query 或 action 阶段的 stale element 都只触发一次完整重新 materialize。
        for attempt in 0..=1 {
            let root = match self.root_element(request.window, &budget) {
                Ok(root) => root,
                Err(error) if attempt == 0 && error.is_element_unavailable() => continue,
                Err(error) => return Err(error.into_automation_error()),
            };
            let candidates = match self.execute_expression(
                &root,
                SearchScope::Subtree,
                &request.plan.query.expression,
                &mut budget,
            ) {
                Ok(candidates) => candidates,
                Err(error) if attempt == 0 && error.is_element_unavailable() => continue,
                Err(error) => return Err(error.into_automation_error()),
            };
            let target = resolve_unique(candidates, &request.query)?;
            budget
                .check_deadline()
                .map_err(UiaError::into_automation_error)?;
            match execute_action(&target.element, &request.plan.action) {
                Ok(executed) => {
                    return Ok(ActionOutcome {
                        backend: BackendKind::WindowsUia,
                        message: executed.message.to_owned(),
                        outputs: executed.outputs,
                    });
                }
                Err(error) if attempt == 0 && error.is_element_unavailable() => continue,
                Err(error) => return Err(error.into_automation_error()),
            }
        }
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::WindowsUia,
            message: "UI Automation target remained stale after one retry".to_owned(),
        })
    }

    /// 校验 handle reuse，并从冻结 HWND 创建本次解析的根元素。
    fn root_element(
        &self,
        window: PreparedWindowTarget,
        budget: &UiaBudgetTracker,
    ) -> Result<IUIAutomationElement, UiaError> {
        budget.check_deadline()?;
        let hwnd = HWND(window.handle as usize as *mut c_void);
        // SAFETY: HWND 仅作为不透明值校验，不被解引用；来源是 prepare 冻结的整数句柄。
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(UiaError::WindowUnavailable {
                message: "prepared HWND no longer identifies a window".to_owned(),
            });
        }
        let mut current_process_id = 0_u32;
        // SAFETY: process id 指针在同步调用期间有效且独占，HWND 已通过 IsWindow 校验。
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut current_process_id));
        }
        if current_process_id == 0 || current_process_id != window.process_id {
            return Err(UiaError::WindowUnavailable {
                message: format!(
                    "prepared HWND process changed from {} to {}",
                    window.process_id, current_process_id
                ),
            });
        }
        // SAFETY: automation client 与返回元素始终留在当前 UIA worker apartment。
        let root = unsafe { self.automation.ElementFromHandle(hwnd) }
            .map_err(|source| UiaError::from_native(UiaOperation::ElementFromHandle, source))?;
        budget.check_deadline()?;
        Ok(root)
    }

    /// 递归执行 Match、关系、Any 与显式选择语义。
    fn execute_expression(
        &self,
        root: &IUIAutomationElement,
        scope: SearchScope,
        expression: &UiaPlanExpr,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        match expression {
            UiaPlanExpr::Match(matcher) => self.execute_match(root, scope, matcher, budget),
            UiaPlanExpr::Descendant { ancestor, target } => {
                let ancestors = self.execute_expression(root, scope, ancestor, budget)?;
                self.execute_within(ancestors, SearchScope::Descendants, target, budget)
            }
            UiaPlanExpr::Child { parent, target } => {
                let parents = self.execute_expression(root, scope, parent, budget)?;
                self.execute_within(parents, SearchScope::Children, target, budget)
            }
            UiaPlanExpr::Any(branches) => execute_any(branches, |branch| {
                self.execute_expression(root, scope, branch, budget)
            }),
            UiaPlanExpr::First(query) => {
                let results = self.execute_expression(root, scope, query, budget)?;
                Ok(results.into_iter().take(1).collect())
            }
            UiaPlanExpr::Nth { query, index } => {
                let results = self.execute_expression(root, scope, query, budget)?;
                Ok(results.into_iter().nth(*index - 1).into_iter().collect())
            }
        }
    }

    /// 对每个关系父节点使用 UIA 自带的严格 Children/Descendants scope。
    fn execute_within(
        &self,
        roots: Vec<ResolvedElement>,
        scope: SearchScope,
        expression: &UiaPlanExpr,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.observe_relation_roots(roots.len())?;
        let mut combined = Vec::new();
        for root in roots {
            budget.check_deadline()?;
            let results = self.execute_expression(&root.element, scope, expression, budget)?;
            append_unique(&mut combined, results);
        }
        Ok(combined)
    }

    /// 使用原生 condition + FindAllBuildCache + 本地 residual 执行 matcher。
    fn execute_match(
        &self,
        root: &IUIAutomationElement,
        scope: SearchScope,
        matcher: &UiaMatcherPlan,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        let condition = build_match_condition(self.automation, matcher)?;
        let cache = build_cache_request(self.automation, &matcher.cache)?;
        // SAFETY: root、condition 与 cache 均在同一 apartment 创建，scope 来自封闭枚举。
        let elements = unsafe { root.FindAllBuildCache(scope.native(), &condition, &cache) }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        budget.check_deadline()?;
        self.collect_matches(elements, &matcher.residual, budget)
    }

    /// 保持 provider encounter order，过滤 residual 并绑定 runtime id。
    fn collect_matches(
        &self,
        elements: IUIAutomationElementArray,
        residual: &[super::native::UiaResidualPredicate],
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        // SAFETY: element array 没有离开创建它的 UIA worker apartment。
        let length = unsafe { elements.Length() }
            .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
        let candidate_count = usize::try_from(length)
            .map_err(|_| UiaError::InvalidCandidateCount { count: length })?;
        budget.observe_candidates(candidate_count)?;
        let mut matches = Vec::new();
        for index in 0..length {
            budget.check_deadline()?;
            // SAFETY: index 来自同一 array 返回的半开区间，array 仍在当前 apartment。
            let element = unsafe { elements.GetElement(index) }
                .map_err(|source| UiaError::from_native(UiaOperation::FindAll, source))?;
            if matches_residual(&element, residual)? {
                let resolved = ResolvedElement {
                    runtime_id: runtime_id(&element)?,
                    element,
                };
                append_unique(&mut matches, [resolved]);
            }
        }
        Ok(matches)
    }
}

/// 懒执行 `any` 分支并原样返回首个非空集合，保留该分支内部的歧义语义。
fn execute_any<TBranch, TResult, TError>(
    branches: &[TBranch],
    mut execute: impl FnMut(&TBranch) -> Result<Vec<TResult>, TError>,
) -> Result<Vec<TResult>, TError> {
    for branch in branches {
        let results = execute(branch)?;
        if !results.is_empty() {
            return Ok(results);
        }
    }
    Ok(Vec::new())
}

/// 当前表达式相对于根元素使用的原生 TreeScope。
#[derive(Clone, Copy)]
enum SearchScope {
    /// 初始查询包含冻结 HWND 对应的根元素。
    Subtree,
    /// 关系查询的严格后代。
    Descendants,
    /// 关系查询的直接子元素。
    Children,
}

impl SearchScope {
    /// 返回 Windows UIA 的原生 scope 常量。
    const fn native(self) -> TreeScope {
        match self {
            Self::Subtree => TreeScope_Subtree,
            Self::Descendants => TreeScope_Descendants,
            Self::Children => TreeScope_Children,
        }
    }
}

/// 只在 worker thread 生命周期内存在的 COM 元素及稳定去重键。
struct ResolvedElement {
    /// UIA COM 元素，不允许穿过 runtime channel。
    element: IUIAutomationElement,
    /// provider 返回的 UIA runtime id。
    runtime_id: Vec<i32>,
}

/// 统一执行 0/1/多目标解析，只有 first/nth 可提前收窄结果。
fn resolve_unique(
    candidates: Vec<ResolvedElement>,
    query: &str,
) -> Result<ResolvedElement, AutomationError> {
    match candidates.len() {
        0 => Err(AutomationError::TargetNotFound {
            query: query.to_owned(),
        }),
        1 => candidates
            .into_iter()
            .next()
            .ok_or_else(|| AutomationError::TargetNotFound {
                query: query.to_owned(),
            }),
        matches => Err(AutomationError::AmbiguousTarget {
            query: query.to_owned(),
            matches,
        }),
    }
}

/// 按 UIA runtime id 去重，同时保留首次出现顺序。
fn append_unique(
    destination: &mut Vec<ResolvedElement>,
    elements: impl IntoIterator<Item = ResolvedElement>,
) {
    for element in elements {
        if !contains_runtime_id(
            destination
                .iter()
                .map(|existing| existing.runtime_id.as_slice()),
            &element.runtime_id,
        ) {
            destination.push(element);
        }
    }
}

/// 判断当前结果集合是否已包含相同 runtime id。
fn contains_runtime_id<'id>(
    existing: impl IntoIterator<Item = &'id [i32]>,
    candidate: &[i32],
) -> bool {
    existing
        .into_iter()
        .any(|runtime_id| runtime_id == candidate)
}

/// 安全读取并拥有 provider 返回的一维整数 SAFEARRAY。
fn runtime_id(element: &IUIAutomationElement) -> Result<Vec<i32>, UiaError> {
    // SAFETY: element 留在 UIA worker；返回 SAFEARRAY 立即交给本函数的唯一 guard。
    let array = unsafe { element.GetRuntimeId() }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    if array.is_null() {
        return Err(UiaError::InvalidRuntimeId);
    }
    let array = SafeArrayGuard(array);
    // SAFETY: guard 持有非空、由 GetRuntimeId 返回且尚未释放的 SAFEARRAY。
    if unsafe { SafeArrayGetDim(array.0) } != 1 {
        return Err(UiaError::InvalidRuntimeId);
    }
    // SAFETY: 已验证数组是一维，维度索引 1 符合 SAFEARRAY API 约定。
    let lower = unsafe { SafeArrayGetLBound(array.0, 1) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    // SAFETY: 已验证数组是一维，维度索引 1 符合 SAFEARRAY API 约定。
    let upper = unsafe { SafeArrayGetUBound(array.0, 1) }
        .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
    if upper < lower {
        return Err(UiaError::InvalidRuntimeId);
    }
    let mut id = Vec::with_capacity((upper - lower + 1) as usize);
    for index in lower..=upper {
        let mut value = 0_i32;
        // SAFETY: index 已被上下界约束，输出指针指向有效且独占的 i32。
        unsafe { SafeArrayGetElement(array.0, &index, (&mut value as *mut i32).cast::<c_void>()) }
            .map_err(|source| UiaError::from_native(UiaOperation::ReadRuntimeId, source))?;
        id.push(value);
    }
    Ok(id)
}

/// 确保 runtime id SAFEARRAY 在同一 apartment 内释放。
struct SafeArrayGuard(*mut SAFEARRAY);

impl Drop for SafeArrayGuard {
    fn drop(&mut self) {
        // SAFETY: 指针来自 GetRuntimeId 且只由本 guard 拥有；释放仍发生在 UIA worker。
        let _ = unsafe { SafeArrayDestroy(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{contains_runtime_id, execute_any};

    /// 验证与 COM 元素无关的 runtime id 顺序去重规则。
    #[test]
    fn runtime_id_membership_uses_the_complete_identifier() {
        let ids = [vec![1, 2], vec![3, 4]];

        assert!(contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 2]));
        assert!(!contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 3]));
    }

    /// `any` 不执行首个非空结果之后的分支，也不把首分支多结果偷偷收窄成一个。
    #[test]
    fn any_stops_at_the_first_non_empty_branch_and_preserves_ambiguity() {
        let executions = Cell::new(0_usize);
        let branches = [vec![1, 2], vec![3]];

        let results = execute_any(&branches, |branch| {
            executions.set(executions.get() + 1);
            Ok::<_, ()>(branch.clone())
        })
        .expect("pure any branch evaluation should succeed");

        assert_eq!(results, vec![1, 2]);
        assert_eq!(executions.get(), 1);
    }

    /// 空分支按声明顺序推进，全部为空时保持 TargetNotFound 所需的空集合。
    #[test]
    fn any_advances_past_empty_branches() {
        let branches = [Vec::<i32>::new(), vec![7]];
        let results = execute_any(&branches, |branch| Ok::<_, ()>(branch.clone()))
            .expect("pure any branch evaluation should succeed");

        assert_eq!(results, vec![7]);
    }
}
