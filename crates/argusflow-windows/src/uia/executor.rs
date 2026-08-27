//! HWND/PID 校验后的进程级 UIA 查询、关系解析与有限 stale 重试。

use std::ffi::c_void;

use argusflow_core::{ActionOutcome, AutomationError, BackendKind};
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{
            IUIAutomation, IUIAutomationElement, TreeScope_Children, TreeScope_Descendants,
        },
        WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    },
};

use super::{
    action::execute_action,
    budget::{UiaBudgetTracker, UiaExecutionBudget},
    cache::build_cache_request,
    condition::build_match_condition,
    element_search::find_cached_matches,
    error::{UiaError, UiaOperation},
    extract::{ExtractExecutionError, execute_extract},
    plan::{UiaMatcherPlan, UiaPlanExpr, UiaResultLimit},
    process_search::find_process_matches,
    runtime::{PreparedWindowTarget, UiaExecuteRequest},
    target_selection::{
        ResolvedElement, ResolvedElementSet, TargetSelectionError, resolution_error,
        resolve_action_target,
    },
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
        // 遍历/关系上限只约束本次 materialize；业务目标等待由 PreparedPlan 统一编排。
        let mut materialization_budget = UiaBudgetTracker::new(budget);
        self.execute_once(&request, &mut materialization_budget)
    }

    /// 使用同一冻结计划完成一次 materialize、动作适配与执行。
    fn execute_once(
        &self,
        request: &UiaExecuteRequest,
        budget: &mut UiaBudgetTracker,
    ) -> Result<ActionOutcome, AutomationError> {
        // root、query 或 action 阶段的 stale element 都只触发一次完整重新 materialize。
        for attempt in 0..=1 {
            let root = match self.root_element(request.window, &*budget) {
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
                UiaResultLimit::All,
                &mut *budget,
            ) {
                Ok(candidates) => candidates,
                Err(error) if attempt == 0 && error.is_element_unavailable() => continue,
                Err(error) => return Err(error.into_automation_error()),
            };
            if let super::plan::UiaActionPlan::Extract {
                cardinality,
                fields,
            } = &request.plan.action
            {
                budget
                    .check_deadline()
                    .map_err(UiaError::into_automation_error)?;
                match execute_extract(candidates, *cardinality, fields) {
                    Ok(outcome) => return Ok(outcome),
                    Err(ExtractExecutionError::Uia(error))
                        if attempt == 0 && error.is_element_unavailable() =>
                    {
                        continue;
                    }
                    Err(ExtractExecutionError::Uia(error)) => {
                        return Err(error.into_automation_error());
                    }
                    Err(ExtractExecutionError::Resolution(failure)) => {
                        return Err(resolution_error(failure, &request.query));
                    }
                }
            }
            let target = match resolve_action_target(candidates, &request.plan.action) {
                Ok(target) => target,
                Err(TargetSelectionError::Uia(error))
                    if attempt == 0 && error.is_element_unavailable() =>
                {
                    continue;
                }
                Err(TargetSelectionError::Uia(error)) => {
                    return Err(error.into_automation_error());
                }
                Err(TargetSelectionError::Resolution(failure)) => {
                    return Err(resolution_error(failure, &request.query));
                }
            };
            budget
                .check_deadline()
                .map_err(UiaError::into_automation_error)?;
            match execute_action(
                &target.element.element,
                &request.plan.action,
                target.strategy,
            ) {
                Ok(executed) => {
                    return Ok(ActionOutcome {
                        backend: BackendKind::WindowsUia,
                        message: executed.message.to_owned(),
                        outputs: executed.outputs,
                        diagnostic_evidence: Vec::new(),
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
        result_limit: UiaResultLimit,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        match expression {
            UiaPlanExpr::Match(matcher) => {
                self.execute_match(root, scope, matcher, result_limit, budget)
            }
            UiaPlanExpr::Descendant { ancestor, target } => {
                let ancestors =
                    self.execute_expression(root, scope, ancestor, UiaResultLimit::All, budget)?;
                self.execute_within(
                    ancestors,
                    SearchScope::Descendants,
                    target,
                    result_limit,
                    budget,
                )
            }
            UiaPlanExpr::Child { parent, target } => {
                let parents =
                    self.execute_expression(root, scope, parent, UiaResultLimit::All, budget)?;
                self.execute_within(parents, SearchScope::Children, target, result_limit, budget)
            }
            UiaPlanExpr::First(query) => {
                self.execute_expression(root, scope, query, UiaResultLimit::first(), budget)
            }
            UiaPlanExpr::Nth { query, index } => {
                let results = self.execute_expression(
                    root,
                    scope,
                    query,
                    UiaResultLimit::at_most(*index),
                    budget,
                )?;
                Ok(results
                    .into_iter()
                    .nth(index.get() - 1)
                    .into_iter()
                    .collect())
            }
        }
    }

    /// 对每个关系父节点使用原生 TreeScope 保持严格 Children/Descendants scope。
    fn execute_within(
        &self,
        roots: Vec<ResolvedElement>,
        scope: SearchScope,
        expression: &UiaPlanExpr,
        result_limit: UiaResultLimit,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.observe_relation_roots(roots.len())?;
        let mut combined = ResolvedElementSet::new();
        for root in roots {
            budget.check_deadline()?;
            if result_limit.is_reached(combined.len()) {
                break;
            }
            // 首个关系根没有跨根重复，可以安全下推 First/Nth 限制；后续根可能先返回
            // 已出现的 runtime id，必须完整扫描当前根后再按唯一结果数量截断。
            let root_limit = if combined.is_empty() {
                result_limit
            } else {
                UiaResultLimit::All
            };
            let results =
                self.execute_expression(&root.element, scope, expression, root_limit, budget)?;
            combined.extend_until(results, result_limit);
        }
        Ok(combined.into_vec())
    }

    /// 使用原生 condition、BuildCache 与本地 residual 执行 matcher。
    fn execute_match(
        &self,
        root: &IUIAutomationElement,
        scope: SearchScope,
        matcher: &UiaMatcherPlan,
        result_limit: UiaResultLimit,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        budget.check_deadline()?;
        if let SearchScope::Process { process_id } = scope {
            return self.execute_process_match(process_id, matcher, result_limit, budget);
        }
        let native_scope = match scope {
            SearchScope::Descendants => TreeScope_Descendants,
            SearchScope::Children => TreeScope_Children,
            SearchScope::Process { .. } => unreachable!("process scope returned above"),
        };
        let condition = build_match_condition(self.automation, matcher)?;
        let cache = build_cache_request(self.automation, &matcher.cache)?;
        find_cached_matches(
            self.automation,
            root,
            native_scope,
            &condition,
            &cache,
            &matcher.residual,
            result_limit,
            budget,
        )
    }

    /// 从目标进程各顶层窗口的独立 UIA 子树中解析并按 runtime id 去重。
    fn execute_process_match(
        &self,
        process_id: u32,
        matcher: &UiaMatcherPlan,
        result_limit: UiaResultLimit,
        budget: &mut UiaBudgetTracker,
    ) -> Result<Vec<ResolvedElement>, UiaError> {
        find_process_matches(self.automation, process_id, matcher, result_limit, budget)
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
