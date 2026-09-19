//! 单次副作用加只读后置验证；不重试复制、粘贴或发送。
use super::{clipboard, compiler::Kind};
use crate::{AutomationHost, resources::*};
use argusflow_aql::{Attribute, Bindings, CompiledQuery, Value as AqlValue};
use argusflow_runtime::*;
use argusflow_workflow::{ErrorKind, Value, ValueType as Ty};
use std::{sync::Arc, time::Duration};

pub(super) struct TransferTask {
    pub kind: Kind,
    pub query: Option<CompiledQuery>,
    pub host: Arc<AutomationHost>,
}
impl PreparedTask for TransferTask {
    fn signature(&self) -> TaskSignature {
        let mut s = TaskSignature::default();
        match self.kind {
            Kind::BrowserCopy => {
                s.inputs = [
                    ("expected".into(), Ty::Text),
                    ("start".into(), Ty::Int),
                    ("end".into(), Ty::Int),
                ]
                .into();
                s.resources = [("page".into(), PAGE.into())].into();
                s.outputs = [("text".into(), Ty::Text), ("sequence".into(), Ty::Int)].into();
            }
            Kind::Checkpoint => {
                s.outputs.insert("sequence".into(), Ty::Int);
            }
            Kind::ClipboardWait => {
                s.inputs = [("expected".into(), Ty::Text), ("previous".into(), Ty::Int)].into();
                s.outputs.insert("sequence".into(), Ty::Int);
            }
            Kind::DocumentWait => {
                s.inputs.insert("expected".into(), Ty::Text);
                s.resources.insert("window".into(), WINDOW.into());
            }
        }
        s
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let mut output = TaskOutput::default();
            match self.kind {
                Kind::Checkpoint => {
                    output.values.insert(
                        "sequence".into(),
                        Value::Int(i64::from(
                            clipboard::read(&self.host, &context).await?.sequence,
                        )),
                    );
                }
                Kind::ClipboardWait => {
                    let found = clipboard::wait(
                        &self.host,
                        &context,
                        number(&context, "previous")?,
                        text(&context, "expected")?,
                    )
                    .await?;
                    output
                        .values
                        .insert("sequence".into(), Value::Int(i64::from(found.sequence)));
                }
                Kind::BrowserCopy => {
                    let page = &resource::<PageResource>(&context, "page")?.page;
                    let query = bind(&self.query)?;
                    let matches = page
                        .query_aql(&query, context.operation)
                        .await
                        .map_err(native)?;
                    let [target] = matches.as_slice() else {
                        return Err(RunError::new(
                            ErrorKind::Ambiguous,
                            "浏览器文本定位必须唯一",
                        ));
                    };
                    let before = clipboard::read(&self.host, &context).await?.sequence;
                    let selected = target
                        .copy_text_range(
                            text(&context, "expected")?,
                            number(&context, "start")?,
                            number(&context, "end")?,
                            context.operation,
                        )
                        .await
                        .map_err(native)?;
                    let after = clipboard::wait(&self.host, &context, before, &selected).await?;
                    output.values.insert("text".into(), Value::Text(selected));
                    output
                        .values
                        .insert("sequence".into(), Value::Int(i64::from(after.sequence)));
                }
                Kind::DocumentWait => {
                    let window = &resource::<WindowResource>(&context, "window")?.0;
                    let uia = self
                        .host
                        .uia
                        .as_ref()
                        .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "UIA 未装配"))?;
                    let expected = normalize(text(&context, "expected")?);
                    let input =
                        self.host.input.as_ref().ok_or_else(|| {
                            RunError::new(ErrorKind::Unavailable, "输入服务未装配")
                        })?;
                    input
                        .perform_operation(
                            window.clone(),
                            argusflow_windows::InputAction::ActivateWindow,
                            context.operation,
                        )
                        .await
                        .map_err(argusflow_core::Failure::from)?;
                    loop {
                        window
                            .require_foreground()
                            .map_err(argusflow_core::Failure::from)?;
                        let query = bind(&self.query)?;
                        let matches = uia
                            .query_aql(window.clone(), query, context.operation)
                            .await
                            .map_err(argusflow_core::Failure::from)?;
                        let actual = match matches.as_slice() {
                            [m] => match m.attributes().get(&Attribute::Text) {
                                Some(AqlValue::Text(t)) => Some(normalize(t)),
                                _ => None,
                            },
                            _ => None,
                        };
                        for item in matches {
                            item.handle().release();
                        }
                        if actual.as_ref() == Some(&expected) {
                            break;
                        }
                        context.operation.check("document_postcondition")?;
                        tokio::time::sleep(
                            Duration::from_millis(50).min(context.operation.remaining()),
                        )
                        .await;
                    }
                }
            }
            Ok(output)
        })
    }
}
fn normalize(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}
fn number(context: &TaskContext<'_>, name: &str) -> Result<u32, RunError> {
    match context.inputs.get(name) {
        Some(Value::Int(n)) => u32::try_from(*n)
            .map_err(|_| RunError::new(ErrorKind::Expression, "范围或序号超出 u32")),
        _ => Err(RunError::new(ErrorKind::Contract, "需要整数")),
    }
}
fn native(error: argusflow_browser::BrowserError) -> RunError {
    argusflow_core::Failure::from(error).into()
}
fn bind(query: &Option<CompiledQuery>) -> Result<argusflow_aql::BoundQuery, RunError> {
    query
        .as_ref()
        .ok_or_else(|| RunError::new(ErrorKind::Contract, "缺少查询"))?
        .bind(&Bindings::new())
        .map_err(|e| RunError::new(ErrorKind::Expression, e.to_string()))
}
