//! OCR 工作流节点仅装配窗口范围，模型与采集服务由宿主共享管理。
use crate::{AutomationHost, resources::*};
use argusflow_core::Failure;
use argusflow_runtime::*;
use argusflow_workflow::ErrorKind;
use std::sync::Arc;
pub(crate) struct OcrCompiler {
    pub host: Arc<AutomationHost>,
}
impl NodeCompiler for OcrCompiler {
    fn type_id(&self) -> &str {
        "window.ocr"
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        if version != 1 || config.as_object().is_none_or(|o| !o.is_empty()) {
            return Err("OCR 范围要求版本 1 和空配置".into());
        }
        Ok(Arc::new(OcrTask(self.host.clone())))
    }
}
struct OcrTask(Arc<AutomationHost>);
impl PreparedTask for OcrTask {
    fn signature(&self) -> TaskSignature {
        TaskSignature {
            resources: [("window".into(), WINDOW.into())].into(),
            resource_outputs: [("source".into(), SOURCE.into())].into(),
            ..Default::default()
        }
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let window = resource::<WindowResource>(&context, "window")?.0.clone();
            window.require_foreground().map_err(Failure::from)?;
            let input = self
                .0
                .input
                .clone()
                .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "输入服务未装配"))?;

            let session = self.0.ocr.acquire(context.operation).await?;
            let binding = OcrWindow {
                session,
                window,
                input,
            };
            binding.ready(context.operation).await?;
            let mut output = TaskOutput::default();
            output.resources.insert(
                "source".into(),
                Arc::new(QuerySourceResource::provider(Arc::new(binding))),
            );
            Ok(output)
        })
    }
}
