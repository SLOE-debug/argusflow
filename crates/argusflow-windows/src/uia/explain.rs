use argusflow_agent::{PlanStepExplain, PlanStepKind};

use super::UiaPlanExpr;

/// 从真实 UIA 逻辑计划递归生成开发者 Explain 步骤。
pub(super) fn explain_uia_plan(expression: &UiaPlanExpr) -> Vec<PlanStepExplain> {
    let mut steps = Vec::new();
    visit(expression, &mut steps);
    steps
}

/// 按执行顺序遍历计划表达式。
fn visit(expression: &UiaPlanExpr, steps: &mut Vec<PlanStepExplain>) {
    match expression {
        UiaPlanExpr::Match(matcher) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::CandidateSource,
                summary: format!("UIA ControlType::{:?}", matcher.role),
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
        UiaPlanExpr::Any(queries) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Traversal,
                summary: format!("{} executable any branch(es)", queries.len()),
            });
            for query in queries {
                visit(query, steps);
            }
        }
        UiaPlanExpr::Not(query) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Traversal,
                summary: "exclude result set".to_owned(),
            });
            visit(query, steps);
        }
        UiaPlanExpr::First(query) => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Selection,
                summary: "first result".to_owned(),
            });
            visit(query, steps);
        }
        UiaPlanExpr::Nth { query, index } => {
            steps.push(PlanStepExplain {
                kind: PlanStepKind::Selection,
                summary: format!("result #{index}"),
            });
            visit(query, steps);
        }
    }
}
