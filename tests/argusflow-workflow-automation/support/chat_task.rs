//! 仅此 demo 注册的组合任务；明确复用现有微信执行示例，不冒充内置通用节点。
use argusflow_runtime::{
    NodeCompiler, NodeRegistry, PreparedTask, RunError, TaskContext, TaskFuture, TaskOutput,
    TaskSignature,
};
use argusflow_workflow::{ErrorKind, Value, ValueType};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct ChatCompiler {
    directory: Option<PathBuf>,
    sequence: Arc<AtomicUsize>,
}
struct ChatTask {
    directory: Option<PathBuf>,
    sequence: Arc<AtomicUsize>,
}
pub fn register(registry: &mut NodeRegistry, directory: Option<PathBuf>) -> Result<(), String> {
    registry.register(Arc::new(ChatCompiler {
        directory,
        sequence: Arc::new(AtomicUsize::new(1)),
    }))
}
impl NodeCompiler for ChatCompiler {
    fn type_id(&self) -> &str {
        "demo.wechat_paste_send"
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        if version != 1 || config.as_object().is_none_or(|v| !v.is_empty()) {
            return Err("微信 demo 任务仅接受 version=1、空 config".into());
        }
        Ok(Arc::new(ChatTask {
            directory: self.directory.clone(),
            sequence: self.sequence.clone(),
        }))
    }
}
impl PreparedTask for ChatTask {
    fn signature(&self) -> TaskSignature {
        TaskSignature {
            inputs: [
                ("recipient".into(), ValueType::Text),
                ("expected".into(), ValueType::Text),
            ]
            .into(),
            outputs: [("receipt".into(), ValueType::Text)].into(),
            ..Default::default()
        }
    }
    fn execute<'a>(&'a self, context: TaskContext<'a>) -> TaskFuture<'a, TaskOutput> {
        Box::pin(async move {
            let get = |name: &str| match context.inputs.get(name) {
                Some(Value::Text(text)) => Ok(text.clone()),
                _ => Err(RunError::new(ErrorKind::Contract, "需要文本输入")),
            };
            let recipient = get("recipient")?;
            let expected = get("expected")?;
            let directory = self
                .directory
                .as_ref()
                .ok_or_else(|| RunError::new(ErrorKind::Unavailable, "验证宿主禁止真实发送"))?;
            if recipient != "文件传输助手" {
                return Err(RunError::new(
                    ErrorKind::Contract,
                    "demo 仅授权文件传输助手",
                ));
            }
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let mut command = tokio::process::Command::new(
                root.join("target/debug/examples/observation_demo.exe"),
            );
            command
                .args(["paste-send", &recipient, &expected])
                .current_dir(&root)
                .kill_on_drop(true);
            context.operation.begin_effect("wechat_demo_paste_send")?;
            let result = command
                .output()
                .await
                .map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()))?;
            let receipt = serde_json::json!({"recipient":recipient,"expected":expected,"process_succeeded":result.status.success(),"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)});
            std::fs::write(
                directory.join(format!(
                    "wechat-{}.json",
                    self.sequence.fetch_add(1, Ordering::Relaxed)
                )),
                serde_json::to_vec_pretty(&receipt)
                    .map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()))?,
            )
            .map_err(|e| RunError::new(ErrorKind::Unavailable, e.to_string()))?;
            if !result.status.success() {
                return Err(RunError::new(
                    ErrorKind::Unavailable,
                    format!(
                        "微信阶段停止，不重发：{} {}",
                        String::from_utf8_lossy(&result.stdout),
                        String::from_utf8_lossy(&result.stderr)
                    ),
                ));
            }
            Ok(TaskOutput {
                values: [(
                    "receipt".into(),
                    Value::Text(String::from_utf8_lossy(&result.stdout).into_owned()),
                )]
                .into(),
                ..Default::default()
            })
        })
    }
}
