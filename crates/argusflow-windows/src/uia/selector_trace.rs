//! UIA snapshot 上的确定性 selector stage 与 near-miss 解释。

use argusflow_query::BranchPath;
use serde::Serialize;

use super::{
    condition::control_type_id,
    evidence::{UiaNodeSnapshot, UiaPatternSnapshot},
    native::{
        UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty, UiaResidualMatcher,
        UiaResidualPredicate, UiaRoleConstraint,
    },
    plan::{UiaMatcherPlan, UiaPlanExpr},
};

/// selector trace 的真实进程搜索边界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SelectorScopeSummary {
    /// 固定 backend 名称。
    backend: &'static str,
    /// Desktop root 下用于硬过滤的进程 ID。
    process_id: u32,
    /// 真实 executor 使用的根范围说明。
    root: &'static str,
}

/// 单个 matcher 从进程节点到完整谓词的数量变化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SelectorTraceStage {
    /// matcher 在冻结表达式中的稳定深度优先序号。
    matcher_index: usize,
    /// 原始进程 Control View 节点数。
    process_candidates: usize,
    /// 角色匹配后的节点数。
    role_matches: usize,
    /// 全部 pushdown 与 residual 条件通过的节点数。
    predicate_matches: usize,
}

/// 一个没有进入执行路径的确定性诊断候选。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct NearMiss {
    /// snapshot 内关联使用的 runtime id。
    runtime_id: Vec<i32>,
    /// Accessible Name。
    name: String,
    /// AutomationId。
    automation_id: String,
    /// AcceleratorKey。
    accelerator_key: String,
    /// AccessKey。
    access_key: String,
    /// provider framework id。
    framework_id: String,
    /// 动作相关 patterns。
    available_patterns: Vec<UiaPatternSnapshot>,
    /// 没有通过的明确属性条件。
    failed_predicates: Vec<String>,
}

/// 真实 matcher 候选及其确定性谓词结论，不会进入动作执行路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct DiagnosticCandidate {
    /// 候选所属 matcher 的稳定深度优先序号。
    matcher_index: usize,
    /// snapshot 内关联使用的 runtime id。
    runtime_id: Vec<i32>,
    /// Accessible Name。
    name: String,
    /// AutomationId。
    automation_id: String,
    /// 是否通过该 matcher 的全部属性谓词。
    matched: bool,
    /// 没有通过的明确属性条件；成功候选为空。
    failed_predicates: Vec<String>,
}

/// 一次失败 candidate 的结构化 selector 解释。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SelectorTrace {
    /// 完整 AQL fallback 路径。
    pub(super) branch_path: BranchPath,
    /// 与 executor 相同的搜索边界。
    scope: SelectorScopeSummary,
    /// 每个 matcher 的过滤统计。
    stages: Vec<SelectorTraceStage>,
    /// 进入角色过滤后的全部真实 matcher 候选及其逐条件结果。
    #[serde(skip)]
    pub(super) candidates: Vec<DiagnosticCandidate>,
    /// 只用于诊断、永远不会自动执行的候选。
    pub(super) near_misses: Vec<NearMiss>,
}

/// 在已冻结 UIA plan 和同一进程 snapshot 上构造 trace。
pub(super) fn build_selector_trace(
    branch_path: BranchPath,
    process_id: u32,
    expression: &UiaPlanExpr,
    nodes: &[UiaNodeSnapshot],
    max_near_misses: usize,
) -> SelectorTrace {
    let mut matchers = Vec::new();
    collect_matchers(expression, &mut matchers);
    let mut stages = Vec::with_capacity(matchers.len());
    let mut candidates = Vec::new();
    let mut near_misses = Vec::new();
    for (matcher_index, matcher) in matchers.into_iter().enumerate() {
        let role_nodes = nodes
            .iter()
            .filter(|node| matches_role(node, matcher.role))
            .collect::<Vec<_>>();
        let mut predicate_matches = 0_usize;
        for node in &role_nodes {
            let failed_predicates = failed_predicates(node, matcher);
            let matched = failed_predicates.is_empty();
            candidates.push(DiagnosticCandidate {
                matcher_index,
                runtime_id: node.runtime_id.clone(),
                name: node.name.clone(),
                automation_id: node.automation_id.clone(),
                matched,
                failed_predicates: failed_predicates.clone(),
            });
            if matched {
                predicate_matches = predicate_matches.saturating_add(1);
            } else if near_misses.len() < max_near_misses {
                near_misses.push(NearMiss {
                    runtime_id: node.runtime_id.clone(),
                    name: node.name.clone(),
                    automation_id: node.automation_id.clone(),
                    accelerator_key: node.accelerator_key.clone(),
                    access_key: node.access_key.clone(),
                    framework_id: node.framework_id.clone(),
                    available_patterns: node.available_patterns.clone(),
                    failed_predicates,
                });
            }
        }
        stages.push(SelectorTraceStage {
            matcher_index,
            process_candidates: nodes.len(),
            role_matches: role_nodes.len(),
            predicate_matches,
        });
    }
    SelectorTrace {
        branch_path,
        scope: SelectorScopeSummary {
            backend: "windows_uia",
            process_id,
            root: "desktop_root_process_fragments",
        },
        stages,
        candidates,
        near_misses,
    }
}

/// 深度优先收集所有关系节点 matcher，保留冻结表达式顺序。
fn collect_matchers<'plan>(
    expression: &'plan UiaPlanExpr,
    destination: &mut Vec<&'plan UiaMatcherPlan>,
) {
    match expression {
        UiaPlanExpr::Match(matcher) => destination.push(matcher),
        UiaPlanExpr::Descendant { ancestor, target } => {
            collect_matchers(ancestor, destination);
            collect_matchers(target, destination);
        }
        UiaPlanExpr::Child { parent, target } => {
            collect_matchers(parent, destination);
            collect_matchers(target, destination);
        }
        UiaPlanExpr::First(query) | UiaPlanExpr::Nth { query, .. } => {
            collect_matchers(query, destination);
        }
    }
}

/// 使用 snapshot 中的原始属性复现 compiler 的角色约束。
fn matches_role(node: &UiaNodeSnapshot, role: UiaRoleConstraint) -> bool {
    match role {
        UiaRoleConstraint::ControlType(control_type) => {
            node.control_type == control_type_id(control_type)
        }
        UiaRoleConstraint::Dialog => {
            node.control_type == control_type_id(super::native::UiaControlType::Window)
                && node.is_dialog
        }
    }
}

/// 返回当前 role candidate 没有通过的全部明确谓词。
fn failed_predicates(node: &UiaNodeSnapshot, matcher: &UiaMatcherPlan) -> Vec<String> {
    matcher
        .pushdown
        .iter()
        .filter(|predicate| !matches_native(node, predicate))
        .map(describe_native)
        .chain(
            matcher
                .residual
                .iter()
                .filter(|predicate| !matches_residual(node, predicate))
                .map(describe_residual),
        )
        .collect()
}

/// 对 snapshot 强类型值执行等值或不等比较。
fn matches_native(node: &UiaNodeSnapshot, predicate: &UiaNativePredicate) -> bool {
    let Some(actual) = snapshot_value(node, predicate.property) else {
        return false;
    };
    match &predicate.comparison {
        UiaNativeComparison::Equal(expected) => actual == *expected,
        UiaNativeComparison::NotEqual(expected) => actual != *expected,
    }
}

/// 对 snapshot 字符串执行 frozen residual matcher。
fn matches_residual(node: &UiaNodeSnapshot, predicate: &UiaResidualPredicate) -> bool {
    let Some(UiaNativeValue::Text(actual)) = snapshot_value(node, predicate.projection.property())
    else {
        return false;
    };
    match &predicate.matcher {
        UiaResidualMatcher::Contains(expected) => actual.contains(expected),
        UiaResidualMatcher::StartsWith(expected) => actual.starts_with(expected),
        UiaResidualMatcher::EndsWith(expected) => actual.ends_with(expected),
        UiaResidualMatcher::Regex(regex) => regex.is_match(&actual),
    }
}

/// 把 node 字段映射回 compiler 的封闭属性值域。
fn snapshot_value(node: &UiaNodeSnapshot, property: UiaProperty) -> Option<UiaNativeValue> {
    Some(match property {
        UiaProperty::Name => UiaNativeValue::Text(node.name.clone()),
        UiaProperty::AutomationId => UiaNativeValue::Text(node.automation_id.clone()),
        UiaProperty::ClassName => UiaNativeValue::Text(node.class_name.clone()),
        UiaProperty::AcceleratorKey => UiaNativeValue::Text(node.accelerator_key.clone()),
        UiaProperty::AccessKey => UiaNativeValue::Text(node.access_key.clone()),
        UiaProperty::FrameworkId => UiaNativeValue::Text(node.framework_id.clone()),
        UiaProperty::Value => UiaNativeValue::Text(node.value.clone()?),
        UiaProperty::IsEnabled => UiaNativeValue::Boolean(node.is_enabled),
        UiaProperty::IsOffscreen => UiaNativeValue::Boolean(node.is_offscreen),
        UiaProperty::HasKeyboardFocus => UiaNativeValue::Boolean(node.has_keyboard_focus),
        UiaProperty::ToggleState => UiaNativeValue::Integer(node.toggle_state),
        UiaProperty::IsSelected => UiaNativeValue::Boolean(node.is_selected),
        UiaProperty::IsDialog => UiaNativeValue::Boolean(node.is_dialog),
    })
}

/// 生成稳定、紧凑的原生谓词说明。
fn describe_native(predicate: &UiaNativePredicate) -> String {
    format!("{:?} {:?}", predicate.property, predicate.comparison)
}

/// 生成稳定、紧凑的 residual 谓词说明。
fn describe_residual(predicate: &UiaResidualPredicate) -> String {
    format!(
        "{:?} {:?}",
        predicate.projection.property(),
        predicate.matcher
    )
}
