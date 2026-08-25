use argusflow_agent::{PlanStepExplain, PlanStepKind};

use super::{CdpCandidateSource, CdpPlanExpr};

/// 从真实 CDP 逻辑计划递归生成开发者 Explain 步骤。
pub(crate) fn explain_cdp_plan(expression: &CdpPlanExpr) -> Vec<PlanStepExplain> {
    let mut steps = Vec::new();
    visit(expression, &mut steps);
    steps
}

/// 按执行顺序遍历计划表达式。
fn visit(expression: &CdpPlanExpr, steps: &mut Vec<PlanStepExplain>) {
    match expression {
        CdpPlanExpr::Match(matcher) => {
            let source = match matcher.source {
                CdpCandidateSource::AccessibilityTree => "Accessibility tree",
                CdpCandidateSource::Dom => "DOM tree",
            };
            steps.push(PlanStepExplain {
                kind: PlanStepKind::CandidateSource,
                summary: format!("{source}: {:?}", matcher.role),
            });
            if !matcher.pushdown.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Pushdown,
                    summary: format!("{} native predicate(s)", matcher.pushdown.len()),
                });
            }
            if !matcher.projected_attributes.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Cache,
                    summary: format!("project {:?}", matcher.projected_attributes),
                });
            }
            if !matcher.residual.is_empty() {
                steps.push(PlanStepExplain {
                    kind: PlanStepKind::Residual,
                    summary: format!("{} residual predicate(s)", matcher.residual.len()),
                });
            }
        }
        CdpPlanExpr::Descendant { ancestor, target } => {
            steps.push(scope("descendant traversal"));
            visit(ancestor, steps);
            visit(target, steps);
        }
        CdpPlanExpr::Child { parent, target } => {
            steps.push(scope("direct child traversal"));
            visit(parent, steps);
            visit(target, steps);
        }
        CdpPlanExpr::Not(query) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Traversal,
                summary: "exclude result set".to_owned(),
            });
            visit(query, steps);
        }
        CdpPlanExpr::First(query) => {
            steps.push(selection("first result"));
            visit(query, steps);
        }
        CdpPlanExpr::Nth { query, index } => {
            steps.push(selection(&format!("result #{index}")));
            visit(query, steps);
        }
        CdpPlanExpr::Css { selector } => steps.push(PlanStepExplain {
            kind: PlanStepKind::Pushdown,
            summary: format!("DOM.querySelectorAll({selector:?})"),
        }),
    }
}

/// 创建搜索范围步骤。
fn scope(summary: &str) -> PlanStepExplain {
    PlanStepExplain {
        kind: PlanStepKind::Scope,
        summary: summary.to_owned(),
    }
}

/// 创建结果选择步骤。
fn selection(summary: &str) -> PlanStepExplain {
    PlanStepExplain {
        kind: PlanStepKind::Selection,
        summary: summary.to_owned(),
    }
}
