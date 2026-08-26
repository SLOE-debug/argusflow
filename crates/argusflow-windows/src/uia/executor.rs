//! HWND/PID 校验后的进程级 UIA 查询、关系解析与有限 stale 重试。

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
            IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement, IUIAutomationTreeWalker,
        },
        WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
};

use super::{
    action::execute_action,
    budget::{UiaBudgetTracker, UiaExecutionBudget},
    cache::build_cache_request,
    current_match::matches_current,
    error::{UiaError, UiaOperation},
    plan::{UiaMatcherPlan, UiaPlanExpr},
    process_search::find_process_matches,
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
                SearchScope::Process {
                    process_id: request.window.process_id,
                },
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

    /// 递归执行 Match、关系与显式选择语义。
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

    /// 对每个关系父节点使用有界 RawView TreeWalker 保持严格 Children/Descendants scope。
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

    /// 使用有界 RawView 遍历、单元素原生 condition 与本地 residual 执行 matcher。
    fn execute_match(
        &self,
        root: &IUIAutomationElement,
        scope: SearchScope,
        matcher: &UiaMatcherPlan,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        if let SearchScope::Process { process_id } = scope {
            return self.execute_process_match(process_id, matcher, budget);
        }
        // 关系查询同样只为真正存在的 residual 谓词创建属性缓存。
        let cache = if matcher.residual.is_empty() {
            None
        } else {
            Some(build_cache_request(self.automation, &matcher.cache)?)
        };
        // SAFETY: automation client 及 walker 都留在当前 UIA worker apartment。
        let walker = unsafe { self.automation.RawViewWalker() }
            .map_err(|source| UiaError::from_native(UiaOperation::NavigateTree, source))?;
        let mut matches = Vec::new();
        let children = self.direct_children(root, &walker, budget)?;
        if matches!(scope, SearchScope::Children) {
            for child in children {
                if let Some(element) =
                    self.match_element(&child, matcher, cache.as_ref(), &matcher.residual, budget)?
                {
                    append_unique(&mut matches, [element]);
                }
            }
            return Ok(matches);
        }

        let mut pending = children;
        pending.reverse();
        while let Some(element) = pending.pop() {
            if let Some(resolved) =
                self.match_element(&element, matcher, cache.as_ref(), &matcher.residual, budget)?
            {
                append_unique(&mut matches, [resolved]);
            }
            let mut children = self.direct_children(&element, &walker, budget)?;
            children.reverse();
            pending.extend(children);
        }
        Ok(matches)
    }

    /// 从目标进程各顶层窗口的独立 UIA 子树中解析并按 runtime id 去重。
    fn execute_process_match(
        &self,
        process_id: u32,
        matcher: &UiaMatcherPlan,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        let elements = find_process_matches(self.automation, process_id, matcher, budget)?;
        let mut matches = Vec::with_capacity(elements.len());
        for element in elements {
            append_unique(
                &mut matches,
                [ResolvedElement {
                    runtime_id: runtime_id(&element)?,
                    element,
                }],
            );
        }
        Ok(matches)
    }

    /// 只在当前元素上执行原生 condition，避免一次性物化完整 subtree 数组。
    fn match_element(
        &self,
        element: &IUIAutomationElement,
        matcher: &UiaMatcherPlan,
        cache: Option<&IUIAutomationCacheRequest>,
        residual: &[super::native::UiaResidualPredicate],
        budget: &mut UiaBudgetTracker,
    ) -> Result<Option<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        if !matches_current(element, matcher)? {
            return Ok(None);
        }
        let element = if let Some(cache) = cache {
            // SAFETY: element 与 cache 同属当前 UIA worker apartment，cache 仅请求 Element scope。
            unsafe { element.BuildUpdatedCache(cache) }
                .map_err(|source| UiaError::from_native(UiaOperation::BuildCache, source))?
        } else {
            element.clone()
        };
        if !matches_residual(&element, residual)? {
            return Ok(None);
        }
        Ok(Some(ResolvedElement {
            runtime_id: runtime_id(&element)?,
            element,
        }))
    }

    /// 按 RawView sibling 顺序枚举直接子元素，并在取得每个节点时执行硬预算检查。
    fn direct_children(
        &self,
        root: &IUIAutomationElement,
        walker: &IUIAutomationTreeWalker,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<IUIAutomationElement>, UiaError> {
        budget.check_deadline()?;
        // SAFETY: root 与 walker 同属当前 worker apartment；空结果由 windows-rs 表示为空错误。
        let mut current = optional_element(unsafe { walker.GetFirstChildElement(root) })?;
        let mut children = Vec::new();
        while let Some(element) = current {
            budget.observe_traversal_nodes(1)?;
            budget.check_deadline()?;
            // SAFETY: element 来自同一 walker；调用只导航到同层后继节点。
            current = optional_element(unsafe { walker.GetNextSiblingElement(&element) })?;
            children.push(element);
        }
        Ok(children)
    }
}

/// 当前表达式相对于根元素使用的原生 TreeScope。
#[derive(Clone, Copy)]
enum SearchScope {
    /// 初始查询覆盖冻结应用进程的全部独立 UIA provider fragment。
    Process {
        /// prepare 阶段通过 HWND 校验过的进程 ID。
        process_id: u32,
    },
    /// 关系查询的严格后代。
    Descendants,
    /// 关系查询的直接子元素。
    Children,
}

/// 把 UIA TreeWalker 的 S_OK + null 结束标记与真正 provider 错误分开。
fn optional_element(
    result: windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, UiaError> {
    match result {
        Ok(element) => Ok(Some(element)),
        Err(source) if source.code().0 == 0 => Ok(None),
        Err(source) => Err(UiaError::from_native(UiaOperation::NavigateTree, source)),
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
    use super::contains_runtime_id;

    /// 验证与 COM 元素无关的 runtime id 顺序去重规则。
    #[test]
    fn runtime_id_membership_uses_the_complete_identifier() {
        let ids = [vec![1, 2], vec![3, 4]];

        assert!(contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 2]));
        assert!(!contains_runtime_id(ids.iter().map(Vec::as_slice), &[1, 3]));
    }
}
