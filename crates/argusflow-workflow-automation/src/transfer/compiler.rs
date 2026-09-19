//! 注册清单与配置编译，不在 prepare 阶段接触桌面。
use super::task::TransferTask;
use crate::AutomationHost;
use argusflow_aql::CompiledQuery;
use argusflow_runtime::{NodeCompiler, NodeRegistry, PreparedTask};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    BrowserCopy,
    Checkpoint,
    ClipboardWait,
    DocumentWait,
}
impl Kind {
    fn id(self) -> &'static str {
        match self {
            Self::BrowserCopy => "browser.copy_text",
            Self::Checkpoint => "clipboard.checkpoint",
            Self::ClipboardWait => "clipboard.wait_text",
            Self::DocumentWait => "window.wait_document",
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
struct Empty {}
struct Compiler {
    kind: Kind,
    host: Arc<AutomationHost>,
}
impl NodeCompiler for Compiler {
    fn type_id(&self) -> &str {
        self.kind.id()
    }
    fn compile(
        &self,
        version: u16,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn PreparedTask>, String> {
        if version != 1 {
            return Err("文本传输任务只接受版本 1".into());
        }
        let query: Option<CompiledQuery> =
            if matches!(self.kind, Kind::BrowserCopy | Kind::DocumentWait) {
                let config: QueryConfig =
                    serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
                // 浏览器专属节点允许 DOM/CSS 定位；通用 UIA 文档节点仍遵循中文目标约束。
                let query = if matches!(self.kind, Kind::BrowserCopy) {
                    let translated = argusflow_aql::translate(&config.query);
                    argusflow_aql::compile(translated.source()).map_err(|e| e.to_string())?
                } else {
                    argusflow_aql::compile_target(&config.query).map_err(|e| e.to_string())?
                };
                if !query.parameters().is_empty() {
                    return Err("传输任务定位使用已验证的静态查询，正文由 inputs 传入".into());
                }
                if matches!(self.kind, Kind::DocumentWait)
                    && !query
                        .bind(&argusflow_aql::Bindings::new())
                        .map_err(|e| e.to_string())?
                        .attributes()
                        .contains(&argusflow_aql::Attribute::Text)
                {
                    return Err(
                        "window.wait_document 查询必须请求文本属性，例如 文档(文本 包含 \"\")"
                            .into(),
                    );
                }
                Some(query)
            } else {
                let _: Empty = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
                None
            };
        Ok(Arc::new(TransferTask {
            kind: self.kind,
            query,
            host: self.host.clone(),
        }))
    }
}
pub(crate) fn register(
    registry: &mut NodeRegistry,
    host: Arc<AutomationHost>,
) -> Result<(), String> {
    for kind in [
        Kind::BrowserCopy,
        Kind::Checkpoint,
        Kind::ClipboardWait,
        Kind::DocumentWait,
    ] {
        registry.register(Arc::new(Compiler {
            kind,
            host: host.clone(),
        }))?;
    }
    Ok(())
}
