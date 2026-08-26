use argusflow_agent::{PlanStepExplain, PlanStepKind};

use super::{UiaActionPlan, UiaActionSupport, UiaPlanExpr, UiaRoleConstraint};

/// 从真实 UIA 逻辑计划递归生成开发者 Explain 步骤。
pub(super) fn explain_uia_plan(expression: &UiaPlanExpr) -> Vec<PlanStepExplain> {
    let mut steps = Vec::new();
    visit(expression, &mut steps);
    steps
}

/// 返回 prepare 阶段冻结的真实 UIA action pattern 说明。
pub(super) fn explain_uia_action(
    action: &UiaActionPlan,
    support: UiaActionSupport,
) -> PlanStepExplain {
    let pattern_check = match support {
        UiaActionSupport::Native => "role-proven",
        UiaActionSupport::RequiresRuntimePatternCheck => "runtime pattern check required",
        UiaActionSupport::Unsupported => "unsupported",
    };
    PlanStepExplain {
        kind: PlanStepKind::Action,
        summary: match action {
            UiaActionPlan::Invoke => {
                format!("Invoke/ExpandCollapse/LegacyIAccessible semantic action ({pattern_check})")
            }
            UiaActionPlan::SetValue { .. } => {
                format!("ValuePattern::SetValue ({pattern_check})")
            }
            UiaActionPlan::GetText => format!("CurrentName ({pattern_check})"),
            UiaActionPlan::GetValue => format!("ValuePattern::CurrentValue ({pattern_check})"),
        },
    }
}

/// 按执行顺序遍历计划表达式。
fn visit(expression: &UiaPlanExpr, steps: &mut Vec<PlanStepExplain>) {
    match expression {
        UiaPlanExpr::Match(matcher) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::CandidateSource,
                summary: match matcher.role {
                    UiaRoleConstraint::ControlType(control_type) => {
                        format!("UIA ControlType::{control_type:?}")
                    }
                    UiaRoleConstraint::Dialog => {
                        "UIA ControlType::Window AND IsDialog=true".to_owned()
                    }
                },
            });
            if !matcher.pushdown.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Pushdown,
                    summary: format!("{} native condition(s)", matcher.pushdown.len()),
                });
            }
            if !matcher.cache.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Cache,
                    summary: format!("CacheRequest {:?}", matcher.cache),
                });
            }
            if !matcher.residual.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Residual,
                    summary: format!("{} residual predicate(s)", matcher.residual.len()),
                });
            }
        }
        UiaPlanExpr::Descendant { ancestor, target } => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Scope,
                summary: "TreeScope::Descendants".to_owned(),
            });
            visit(ancestor, steps);
            visit(target, steps);
        }
        UiaPlanExpr::Child { parent, target } => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Scope,
                summary: "TreeScope::Children".to_owned(),
            });
            visit(parent, steps);
            visit(target, steps);
        }
        UiaPlanExpr::First(query) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Selection,
                summary: "first result (bounded materialization)".to_owned(),
            });
            visit(query, steps);
        }
        UiaPlanExpr::Nth { query, index } => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Selection,
                summary: format!("result #{index} (materialize at most {index})"),
            });
            visit(query, steps);
        }
    }
}
