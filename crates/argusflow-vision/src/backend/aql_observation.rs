//! AQL Vision 计划到观察动作输出的转换。

use std::collections::BTreeMap;

use argusflow_core::{
    ActionOutcome, ActionOutputKey, AutomationAction, AutomationError, BackendKind,
    ExtractCardinality,
};
use serde_json::{Value, json};

use crate::{
    index::VisualSceneSnapshot,
    query::{
        VisionQueryExecutionError, VisionQueryPlan, execute_unique_vision_query,
        execute_vision_query,
    },
};

use super::{outcome_with_contract, project_fields};

/// 把 AQL Vision 计划结果转换为现有观察动作输出。
pub(super) fn execute_aql_observation(
    action: &AutomationAction,
    snapshot: &VisualSceneSnapshot,
    plan: &VisionQueryPlan,
    backend: BackendKind,
) -> Result<ActionOutcome, AutomationError> {
    match action {
        AutomationAction::GetText { .. } => {
            let node = execute_unique_vision_query(snapshot, plan)
                .map_err(|error| map_query_error(plan, error, backend))?;
            let mut outputs = BTreeMap::new();
            outputs.insert(
                ActionOutputKey::Text.as_str().to_owned(),
                Value::String(node.raw_text.clone()),
            );
            outcome_with_contract(action, backend, plan.summary.join("; "), outputs)
        }
        AutomationAction::Extract {
            cardinality,
            fields,
            ..
        } => {
            let result = execute_vision_query(snapshot, plan)
                .map_err(|error| map_query_error(plan, error, backend))?;
            let nodes = match cardinality {
                ExtractCardinality::One => match result.matches.as_slice() {
                    [node] => vec![*node],
                    [] => {
                        return Err(map_query_error(
                            plan,
                            VisionQueryExecutionError::TargetNotFound,
                            backend,
                        ));
                    }
                    nodes => {
                        return Err(map_query_error(
                            plan,
                            VisionQueryExecutionError::TargetAmbiguous {
                                matches: nodes.len(),
                            },
                            backend,
                        ));
                    }
                },
                ExtractCardinality::Many => result.matches,
            };
            let values = nodes
                .iter()
                .map(|node| project_fields(node, fields, backend))
                .collect::<Result<Vec<_>, _>>()?;
            let mut outputs = BTreeMap::new();
            outputs.insert(
                if *cardinality == ExtractCardinality::One {
                    ActionOutputKey::Item.as_str().to_owned()
                } else {
                    ActionOutputKey::Items.as_str().to_owned()
                },
                if *cardinality == ExtractCardinality::One {
                    values.into_iter().next().unwrap_or_else(|| json!({}))
                } else {
                    Value::Array(values)
                },
            );
            outcome_with_contract(action, backend, result.explain.join("; "), outputs)
        }
        _ => Err(AutomationError::BackendUnavailable {
            backend,
            message: "Vision AQL P0 only provides observation actions".to_owned(),
        }),
    }
}

/// 将空间执行错误映射到统一 0/1/N 自动化错误模型。
fn map_query_error(
    plan: &VisionQueryPlan,
    error: VisionQueryExecutionError,
    backend: BackendKind,
) -> AutomationError {
    let query = plan.summary.join("; ");
    match error {
        VisionQueryExecutionError::TargetNotFound | VisionQueryExecutionError::AnchorNotFound => {
            AutomationError::TargetNotFound {
                query,
                details: error.to_string(),
            }
        }
        VisionQueryExecutionError::TargetAmbiguous { matches }
        | VisionQueryExecutionError::AnchorAmbiguous { matches } => {
            AutomationError::AmbiguousTarget {
                query,
                matches,
                details: error.to_string(),
            }
        }
        VisionQueryExecutionError::ObservationIncomplete => AutomationError::BackendFailed {
            backend,
            message: error.to_string(),
        },
    }
}
