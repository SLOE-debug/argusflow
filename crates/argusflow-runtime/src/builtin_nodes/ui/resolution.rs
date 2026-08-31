//! UI 节点动态目标、后置条件与资源作用域的运行时冻结。

use argusflow_core::{
    AppSession, AqlQuery, AutomationError, AutomationExecutionScope, BrowserSession,
    PreparedAqlQuery, PreparedAutomationTarget, PreparedTargetLocator, PreparedVisualPostcondition,
    ResourceTypeId, TargetLocator, TargetScope, UiPostcondition,
};

use crate::{RunContext, RuntimeError};

/// 解析一次 UI 动作的视觉文字表达式，并把结果冻结为运行时定位契约。
pub(super) fn resolve_target(
    target: &argusflow_core::AutomationTarget,
    context: &RunContext,
) -> Result<(argusflow_core::AutomationTarget, PreparedAutomationTarget), RuntimeError> {
    let prepared_locator = match &target.locator {
        TargetLocator::Query { query } => {
            let prepared = resolve_aql_query(query, context)?;
            PreparedTargetLocator::Query {
                query: prepared.query().clone(),
                source: prepared.source().to_owned(),
            }
        }
        TargetLocator::Coordinate { point } => PreparedTargetLocator::Coordinate { point: *point },
        TargetLocator::Focused => PreparedTargetLocator::Focused,
    };
    Ok((
        target.clone(),
        PreparedAutomationTarget::new(
            target.scope.clone(),
            prepared_locator,
            target.backend_policy.clone(),
        ),
    ))
}

/// 解析视觉后置条件中的动态文字表达式与稳定上下文。
pub(super) fn resolve_postcondition(
    postcondition: &Option<UiPostcondition>,
    context: &RunContext,
) -> Result<Option<PreparedVisualPostcondition>, RuntimeError> {
    let Some(postcondition) = postcondition else {
        return Ok(None);
    };
    Ok(Some(match postcondition {
        UiPostcondition::MatchAdded {
            query,
            stable_context,
        } => PreparedVisualPostcondition::MatchAdded {
            query: resolve_aql_query(query, context)?,
            stable_context: stable_context
                .iter()
                .map(|query| resolve_aql_query(query, context))
                .collect::<Result<Vec<_>, _>>()?,
        },
        UiPostcondition::MatchPresent { query } => PreparedVisualPostcondition::MatchPresent {
            query: resolve_aql_query(query, context)?,
        },
    }))
}

/// 解析并冻结一条 AQL 查询及其动态参数，后端不再访问流程表达式环境。
fn resolve_aql_query(
    query: &AqlQuery,
    context: &RunContext,
) -> Result<PreparedAqlQuery, RuntimeError> {
    let parameters = query
        .bindings
        .iter()
        .map(|(name, expression)| {
            context
                .resolve_text(expression)
                .map(|value| (name.clone(), value))
        })
        .collect::<Result<_, _>>()?;
    let parsed = argusflow_query::parse_stored_query(query).map_err(|error| {
        RuntimeError::Automation(AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::OcrSmall,
            message: format!("AQL query could not be prepared: {error}"),
        })
    })?;
    let resolved =
        argusflow_query::resolve_query_parameters(&parsed, &parameters).map_err(|error| {
            RuntimeError::Automation(AutomationError::BackendFailed {
                backend: argusflow_core::BackendKind::OcrSmall,
                message: format!("AQL parameters could not be prepared: {error}"),
            })
        })?;
    Ok(PreparedAqlQuery::new(resolved, query.source.clone()))
}

/// 把资源引用解析成不进入持久化定义的瞬时后端作用域。
pub(super) fn resolve_execution_scope(
    scope: &TargetScope,
    context: &RunContext,
) -> Result<AutomationExecutionScope, RuntimeError> {
    match scope {
        TargetScope::Current => Ok(AutomationExecutionScope::Current),
        TargetScope::Application { resource } => {
            let session = context
                .resources()
                .get::<AppSession>(resource, &ResourceTypeId::application())?;
            let [window] = session.windows.as_slice() else {
                return Err(RuntimeError::ExecutionInvariant(format!(
                    "application resource '{}.{}' does not contain exactly one window",
                    resource.producer_node_id, resource.output_name,
                )));
            };
            Ok(AutomationExecutionScope::Window {
                handle: window.handle,
                process_id: window.process_id,
                capabilities: session.capabilities.clone(),
            })
        }
        TargetScope::Browser { resource } => {
            let session = context
                .resources()
                .get::<BrowserSession>(resource, &ResourceTypeId::browser())?;
            Ok(AutomationExecutionScope::Browser {
                session_id: session.id,
                target_id: session.target_id.clone(),
            })
        }
    }
}
