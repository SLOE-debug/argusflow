//! 执行已编译的目标任务，沿用运行时的取消和超时票据。
use super::{compiler::QueryKind, config::*, snapshot, source};
use crate::{AutomationHost, resources::*};
use argusflow_aql::{Bindings, CompiledQuery, ValueType as AqlType};
use argusflow_automation::Locator;
use argusflow_runtime::*;
use argusflow_workflow::{ErrorKind, Value, ValueType as Ty};
use std::{sync::Arc, time::Duration};
pub(super) struct SourceTask(pub(super) argusflow_automation::QuerySource);
impl PreparedTask for SourceTask {
    fn signature(&self) -> TaskSignature {
        TaskSignature {
            resource_outputs: [("source".into(), SOURCE.into())].into(),
            ..Default::default()
        }
    }
    fn execute<'a>(&'a self, _context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async {
            let mut output = TaskOutput::default();
            output.resources.insert(
                "source".into(),
                Arc::new(QuerySourceResource::new(self.0.clone())),
            );
            Ok(output)
        })
    }
}
pub(super) struct QueryTask {
    pub(super) focused: bool,
    pub(super) keys: Vec<argusflow_core::Key>,
    pub(super) kind: QueryKind,
    pub(super) query: CompiledQuery,
    pub(super) interval_ms: u64,
    pub(super) condition: WaitCondition,
    pub(super) platform: TargetPlatform,
    pub(super) host: Arc<AutomationHost>,
}
impl PreparedTask for QueryTask {
    fn signature(&self) -> TaskSignature {
        let mut s = TaskSignature {
            inputs: self
                .query
                .parameters()
                .iter()
                .map(|(name, ty)| {
                    (
                        name.clone(),
                        match ty {
                            AqlType::Text => Ty::Text,
                            AqlType::Boolean => Ty::Bool,
                            AqlType::Number => Ty::Float,
                        },
                    )
                })
                .collect(),
            resources: [("scope".into(), self.platform.scope_type().into())].into(),
            ..Default::default()
        };
        match self.kind {
            QueryKind::Preview => {
                s.outputs
                    .insert("spatial_preview".into(), super::preview::value_type());
                s.safe_to_retry = true;
            }
            QueryKind::All => {
                s.outputs
                    .insert("matches".into(), Ty::List(Box::new(snapshot::match_type())));
                s.safe_to_retry = true;
            }
            QueryKind::Exists | QueryKind::Wait => {
                s.outputs.insert("exists".into(), Ty::Bool);
                s.safe_to_retry = true;
            }
            QueryKind::Type => {
                s.inputs.insert("text".into(), Ty::Text);
            }
            QueryKind::Click | QueryKind::Bind | QueryKind::Keys => {}
        }
        s
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let mut bindings = Bindings::new();
            for name in self.query.parameters().keys() {
                let value = match context.inputs.get(name) {
                    Some(Value::Text(v)) => argusflow_aql::Value::Text(v.clone()),
                    Some(Value::Bool(v)) => argusflow_aql::Value::Boolean(*v),
                    Some(Value::Float(v)) => argusflow_aql::Value::Number(*v),
                    _ => {
                        return Err(RunError::new(
                            ErrorKind::Contract,
                            "AQL 参数不满足已编译类型",
                        ));
                    }
                };
                bindings.insert(name.clone(), value);
            }
            let locate = || async {
                let source = source::resolve(self.platform, &self.host, &context).await?;
                Locator::bind(source, &self.query, &bindings)
                    .map_err(|_| RunError::new(ErrorKind::Expression, "AQL 参数绑定失败"))
            };
            let mut output = TaskOutput::default();
            match self.kind {
                QueryKind::Preview => {
                    let previews = locate()
                        .await?
                        .preview_with_operation(context.operation)
                        .await
                        .map_err(RunError::from)?;
                    output
                        .values
                        .insert("spatial_preview".into(), super::preview::value(previews));
                }
                QueryKind::Click => locate()
                    .await?
                    .click_with_operation(context.operation)
                    .await
                    .map_err(RunError::from)?,
                QueryKind::Type if self.focused => {
                    locate()
                        .await?
                        .type_focused_with_operation(text(&context, "text")?, context.operation)
                        .await?
                }
                QueryKind::Keys => {
                    locate()
                        .await?
                        .press_keys_with_operation(&self.keys, context.operation)
                        .await?
                }
                QueryKind::Type => locate()
                    .await?
                    .type_text_with_operation(text(&context, "text")?, context.operation)
                    .await
                    .map_err(RunError::from)?,
                QueryKind::All | QueryKind::Exists | QueryKind::Wait => loop {
                    context.operation.check("workflow_query")?;
                    let matches = locate()
                        .await?
                        .find_all_with_operation(context.operation)
                        .await
                        .map_err(RunError::from)?;
                    if matches!(self.kind, QueryKind::All) {
                        let values = matches
                            .iter()
                            .map(snapshot::snapshot)
                            .collect::<Result<Vec<_>, _>>()?;
                        output.values.insert("matches".into(), Value::List(values));
                        break;
                    }
                    let exists = !matches.is_empty();
                    if !matches!(self.kind, QueryKind::Wait)
                        || self.condition.matches(matches.len())
                    {
                        output.values.insert("exists".into(), Value::Bool(exists));
                        break;
                    }
                    // 轮询仅重复无副作用查询，引用票据从不重置总时限。
                    tokio::time::sleep(
                        Duration::from_millis(self.interval_ms).min(context.operation.remaining()),
                    )
                    .await;
                },
                QueryKind::Bind => {
                    return Err(RunError::new(ErrorKind::Contract, "来源绑定不属于查询任务"));
                }
            }
            Ok(output)
        })
    }
}
