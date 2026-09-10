//! 查询在准备阶段编译，运行参数只经类型化 Bindings 输入。
use super::snapshot;
use crate::{AutomationHost, resources::*};
use argusflow_aql::{Bindings, CompiledQuery, ValueType as AqlType};
use argusflow_automation::Locator;
use argusflow_runtime::*;
use argusflow_workflow::{ErrorKind, Value, ValueType as Ty};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy)]
pub(crate) enum QueryKind {
    Bind,
    All,
    Exists,
    Wait,
    Click,
    Type,
}
impl QueryKind {
    pub const ALL: [Self; 6] = [
        Self::Bind,
        Self::All,
        Self::Exists,
        Self::Wait,
        Self::Click,
        Self::Type,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Bind => "source.host",
            Self::All => "aql.query",
            Self::Exists => "aql.exists",
            Self::Wait => "aql.wait",
            Self::Click => "aql.click",
            Self::Type => "aql.type_text",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryConfig {
    query: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitConfig {
    query: String,
    interval_ms: u64,
    present: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindConfig {
    name: String,
}
pub(crate) fn compile(
    kind: QueryKind,
    config: &serde_json::Value,
    host: Arc<AutomationHost>,
) -> Result<Arc<dyn PreparedTask>, String> {
    if matches!(kind, QueryKind::Bind) {
        let config: BindConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        let source = host
            .sources
            .get(&config.name)
            .cloned()
            .ok_or("宿主查询来源未绑定")?;
        return Ok(Arc::new(SourceTask(source)));
    }
    let (query, interval_ms, present) = if matches!(kind, QueryKind::Wait) {
        let config: WaitConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        if config.interval_ms < 10 || config.interval_ms > 60_000 {
            return Err("轮询间隔必须在 10..=60000 毫秒".into());
        }
        (config.query, config.interval_ms, config.present)
    } else {
        let config: QueryConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        (config.query, 0, true)
    };
    let query = argusflow_aql::compile(&query).map_err(|e| e.to_string())?;
    if matches!(kind, QueryKind::Type) && query.parameters().contains_key("text") {
        return Err("text 为输入文字节点保留端口，AQL 参数请使用其他名称".into());
    }
    Ok(Arc::new(QueryTask {
        kind,
        query,
        interval_ms,
        present,
    }))
}
struct SourceTask(argusflow_automation::QuerySource);
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
                Arc::new(QuerySourceResource(self.0.clone())),
            );
            Ok(output)
        })
    }
}
struct QueryTask {
    kind: QueryKind,
    query: CompiledQuery,
    interval_ms: u64,
    present: bool,
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
            resources: [("source".into(), SOURCE.into())].into(),
            ..Default::default()
        };
        match self.kind {
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
            QueryKind::Click | QueryKind::Bind => {}
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
            let source = resource::<QuerySourceResource>(&context, "source")?
                .0
                .clone();
            let locator = Locator::bind(source, &self.query, &bindings)
                .map_err(|_| RunError::new(ErrorKind::Expression, "AQL 参数绑定失败"))?;
            let mut output = TaskOutput::default();
            match self.kind {
                QueryKind::Click => locator
                    .click_with_operation(context.operation)
                    .await
                    .map_err(RunError::from)?,
                QueryKind::Type => locator
                    .type_text_with_operation(text(&context, "text")?, context.operation)
                    .await
                    .map_err(RunError::from)?,
                QueryKind::All | QueryKind::Exists | QueryKind::Wait => loop {
                    context.operation.check("workflow_query")?;
                    let matches = locator
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
                    if !matches!(self.kind, QueryKind::Wait) || exists == self.present {
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
