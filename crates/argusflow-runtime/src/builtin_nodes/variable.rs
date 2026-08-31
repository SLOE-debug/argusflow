use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use argusflow_core::{ValueExpr, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeExecution, NodeFlow, NodeValidationContext, PreparedNode, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode, ValueInput,
};

/// Set Variables 节点中的一个显式赋值。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VariableAssignment {
    /// 要在本次运行变量对象中写入的一级字段名。
    name: String,
    /// 在事务开始时的统一快照上求值的任意 JSON 表达式。
    value: ValueExpr,
}

/// Set Variables 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetVariablesPayload {
    /// 全部成功后一次性提交的赋值集合。
    assignments: Vec<VariableAssignment>,
}

/// 创建冻结的 Set Variables 节点。
pub(super) fn prepare(payload: SetVariablesPayload) -> Arc<dyn PreparedNode> {
    Arc::new(SetVariablesNode {
        assignments: payload.assignments,
    })
}

/// 只通过显式节点修改 Runtime Variables 的事务边界。
#[derive(Debug)]
struct SetVariablesNode {
    /// 已解码但尚未求值的赋值集合。
    assignments: Vec<VariableAssignment>,
}

#[async_trait]
impl PreparedNode for SetVariablesNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        format!("Set {} Variables", self.assignments.len())
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut names = HashSet::new();
        if self.assignments.is_empty() {
            return vec![context.issue(
                ValidationIssueCode::InvalidVariableAssignment,
                "设置变量节点至少需要一项赋值",
            )];
        }
        let declared_variables = context.workflow.variables.as_object();
        let mut issues = Vec::new();
        for assignment in &self.assignments {
            if assignment.name.trim().is_empty() || !names.insert(assignment.name.as_str()) {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidVariableAssignment,
                    "变量名称必须非空且在同一节点内唯一",
                ));
            }
            if !assignment.name.trim().is_empty()
                && !declared_variables
                    .is_some_and(|variables| variables.contains_key(assignment.name.as_str()))
            {
                issues.push(context.issue(
                    ValidationIssueCode::UndeclaredVariable,
                    format!("变量 '{}' 未声明", assignment.name),
                ));
            }
        }
        issues
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        self.assignments
            .iter()
            .map(|assignment| ValueInput::json(&assignment.value))
            .collect()
    }

    async fn execute(
        &self,
        node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let snapshot = context.value_scope(None);
        let mut assignments = BTreeMap::new();
        for assignment in &self.assignments {
            let value = context
                .resolve_in_scope(&assignment.value, &snapshot)
                .map_err(|error| RuntimeError::VariableAssignmentFailed {
                    node_id: node_id.to_owned(),
                    variable: assignment.name.clone(),
                    message: error.to_string(),
                })?;
            assignments.insert(assignment.name.clone(), value);
        }
        context.commit_variables(assignments);
        Ok(NodeExecution::default())
    }
}

#[cfg(test)]
mod tests {
    use argusflow_core::{ValueExpr, ValueSource, WorkflowPermissions};
    use serde_json::{Map, json};
    use uuid::Uuid;

    use super::{SetVariablesNode, VariableAssignment};
    use crate::{PreparedNode, RunContext, RuntimeError};

    #[tokio::test]
    async fn failed_assignment_rolls_back_the_entire_variable_transaction() {
        let node = SetVariablesNode {
            assignments: vec![
                VariableAssignment {
                    name: "state".to_owned(),
                    value: ValueExpr::Literal {
                        value: json!({ "changed": true }),
                    },
                },
                VariableAssignment {
                    name: "missing".to_owned(),
                    value: ValueExpr::Ref {
                        source: ValueSource::Variable {
                            name: "unknown".to_owned(),
                        },
                        pointer: String::new(),
                    },
                },
            ],
        };
        let mut context = RunContext::new(
            Uuid::new_v4(),
            Map::new(),
            json!({ "state": 1 }).as_object().unwrap().clone(),
        );

        assert!(matches!(
            node.execute(
                "set-vars",
                &WorkflowPermissions::default(),
                &mut context,
            )
            .await,
            Err(RuntimeError::VariableAssignmentFailed {
                variable,
                ..
            }) if variable == "missing"
        ));
        assert_eq!(
            context
                .resolve_value(&ValueExpr::Ref {
                    source: ValueSource::Variable {
                        name: "state".to_owned(),
                    },
                    pointer: String::new(),
                })
                .unwrap(),
            json!(1)
        );
    }

    #[tokio::test]
    async fn variable_values_can_change_json_category() {
        let node = SetVariablesNode {
            assignments: vec![VariableAssignment {
                name: "state".to_owned(),
                value: ValueExpr::Literal {
                    value: json!({ "phase": "done" }),
                },
            }],
        };
        let mut context = RunContext::new(
            Uuid::new_v4(),
            Map::new(),
            json!({ "state": 1 }).as_object().unwrap().clone(),
        );

        node.execute("set-vars", &WorkflowPermissions::default(), &mut context)
            .await
            .unwrap();
        assert_eq!(
            context
                .resolve_value(&ValueExpr::Ref {
                    source: ValueSource::Variable {
                        name: "state".to_owned(),
                    },
                    pointer: String::new(),
                })
                .unwrap(),
            json!({ "phase": "done" })
        );
    }
}
