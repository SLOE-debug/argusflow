//! 文件节点仅负责无覆盖预留与读取校验，实际文本编辑由应用完成。
use crate::resources::text;
use argusflow_runtime::*;
use argusflow_workflow::{Value, ValueType};
use std::sync::Arc;

pub(crate) struct FileCompiler {
    pub kind: FileKind,
}
#[derive(Clone, Copy)]
pub(crate) enum FileKind {
    CreateNew,
    WaitText,
}
impl NodeCompiler for FileCompiler {
    fn type_id(&self) -> &str {
        match self.kind {
            FileKind::CreateNew => "file.create_new",
            FileKind::WaitText => "file.wait_text",
        }
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        if version != 1 || config.as_object().is_none_or(|o| !o.is_empty()) {
            return Err("文件节点要求版本 1 和空配置".into());
        }
        Ok(Arc::new(FileTask { kind: self.kind }))
    }
}
struct FileTask {
    kind: FileKind,
}
impl PreparedTask for FileTask {
    fn signature(&self) -> TaskSignature {
        let mut s = TaskSignature::default();
        s.inputs.insert("path".into(), ValueType::Text);
        if matches!(self.kind, FileKind::WaitText) {
            s.inputs.insert("expected".into(), ValueType::Text);
            s.outputs.insert("equal".into(), ValueType::Bool);
            s.safe_to_retry = true;
        }
        s
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            context.operation.check("workflow_file")?;
            let path = text(&context, "path")?;
            let mut output = TaskOutput::default();
            match self.kind {
                FileKind::CreateNew => {
                    super::operations::create_new(path, context.operation).await?
                }
                FileKind::WaitText => {
                    super::operations::wait_text(
                        path,
                        text(&context, "expected")?,
                        context.operation,
                    )
                    .await?;
                    output.values.insert("equal".into(), Value::Bool(true));
                }
            }
            context.operation.check("workflow_file_complete")?;
            Ok(output)
        })
    }
}
