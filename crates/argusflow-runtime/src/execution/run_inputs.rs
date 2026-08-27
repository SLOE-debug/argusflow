use std::collections::HashMap;

use argusflow_core::{RunInputs, WorkflowDefinition, WorkflowInputType};

use crate::error::RuntimeError;

/// 校验一次运行的瞬时输入是否与持久化声明精确一致。
pub(crate) fn validate_run_inputs(
    workflow: &WorkflowDefinition,
    inputs: &RunInputs,
) -> Result<(), RuntimeError> {
    let declarations = workflow
        .inputs
        .iter()
        .map(|input| (input.key.as_str(), input.value_type))
        .collect::<HashMap<_, _>>();

    for (key, value_type) in &declarations {
        let value = inputs
            .values
            .get(*key)
            .ok_or_else(|| RuntimeError::InvalidRunInputs {
                message: format!("required workflow input '{key}' is missing"),
            })?;
        let valid_type = match value_type {
            WorkflowInputType::Text => value.is_string(),
        };
        if !valid_type {
            return Err(RuntimeError::InvalidRunInputs {
                message: format!("workflow input '{key}' must be text"),
            });
        }
    }

    if let Some(unexpected) = inputs
        .values
        .keys()
        .find(|key| !declarations.contains_key(key.as_str()))
    {
        return Err(RuntimeError::InvalidRunInputs {
            message: format!("workflow input '{unexpected}' is not declared"),
        });
    }
    Ok(())
}
