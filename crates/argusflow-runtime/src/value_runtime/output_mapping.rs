use std::collections::BTreeMap;

use argusflow_core::ValueExpr;

use crate::{NodeOutcome, RunContext, RuntimeError};

/// 在原生输出的冻结快照上原子计算并合并节点 Published Outputs。
pub(crate) fn publish_outcome(
    context: &RunContext,
    node_id: &str,
    mut outcome: NodeOutcome,
    bindings: &BTreeMap<String, ValueExpr>,
) -> Result<NodeOutcome, RuntimeError> {
    if bindings.is_empty() {
        return Ok(outcome);
    }
    let snapshot = context.value_scope(Some(&outcome.outputs));
    let mut mapped = BTreeMap::new();
    for (name, expression) in bindings {
        let value = context
            .resolve_in_scope(expression, &snapshot)
            .map_err(|error| RuntimeError::OutputMappingFailed {
                node_id: node_id.to_owned(),
                output_name: name.clone(),
                message: error.to_string(),
            })?;
        mapped.insert(name.clone(), value);
    }
    outcome.outputs.extend(mapped);
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use argusflow_core::{ValueExpr, ValueSource};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::publish_outcome;
    use crate::{NodeOutcome, RunContext, RuntimeError, value_runtime::RuntimeValuePlanBuilder};

    #[test]
    fn custom_outputs_read_native_snapshot_and_can_override_native_names() {
        let path_expression = ValueExpr::Expression {
            source: "result.path".to_owned(),
        };
        let mut builder = RuntimeValuePlanBuilder::default();
        builder.compile(&path_expression).unwrap();
        let context = RunContext::with_value_plan(
            Uuid::new_v4(),
            Default::default(),
            Default::default(),
            builder.finish(),
        );
        let outcome = NodeOutcome::values(BTreeMap::from([
            ("path".to_owned(), json!("C:/before.txt")),
            ("bytes".to_owned(), json!(12)),
        ]));
        let bindings = BTreeMap::from([
            ("output".to_owned(), path_expression),
            (
                "path".to_owned(),
                ValueExpr::Literal {
                    value: json!("C:/overridden.txt"),
                },
            ),
        ]);

        let published = publish_outcome(&context, "write", outcome, &bindings).unwrap();
        assert_eq!(published.outputs["output"], json!("C:/before.txt"));
        assert_eq!(published.outputs["path"], json!("C:/overridden.txt"));
        assert_eq!(published.outputs["bytes"], json!(12));
    }

    #[test]
    fn mapping_is_order_independent_and_failure_publishes_nothing() {
        let derived_expression = ValueExpr::Expression {
            source: "result.first".to_owned(),
        };
        let mut builder = RuntimeValuePlanBuilder::default();
        builder.compile(&derived_expression).unwrap();
        let context = RunContext::with_value_plan(
            Uuid::new_v4(),
            Default::default(),
            Default::default(),
            builder.finish(),
        );
        let bindings = BTreeMap::from([
            ("first".to_owned(), ValueExpr::Literal { value: json!(1) }),
            ("second".to_owned(), derived_expression),
        ]);

        assert!(matches!(
            publish_outcome(
                &context,
                "mapping",
                NodeOutcome::values(BTreeMap::new()),
                &bindings,
            ),
            Err(RuntimeError::OutputMappingFailed {
                output_name,
                ..
            }) if output_name == "second"
        ));
        assert!(matches!(
            context.resolve_value(&ValueExpr::Ref {
                source: ValueSource::Node {
                    node_id: "mapping".to_owned(),
                },
                pointer: String::new(),
            }),
            Err(RuntimeError::ValueUnavailable { .. })
        ));
    }
}
