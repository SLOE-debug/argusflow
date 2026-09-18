//! 节点配置只在准备阶段解码与验证，运行任务接收冻结的配置。
use super::{
    config::*,
    task::{QueryTask, SourceTask},
};
use crate::AutomationHost;
use argusflow_runtime::PreparedTask;
use serde::Deserialize;
use std::sync::Arc;
#[derive(Clone, Copy)]
pub(crate) enum QueryKind {
    Bind,
    All,
    Exists,
    Wait,
    Click,
    Type,
    Preview,
    Keys,
}
impl QueryKind {
    pub const ALL: [Self; 8] = [
        Self::Keys,
        Self::Bind,
        Self::All,
        Self::Exists,
        Self::Wait,
        Self::Click,
        Self::Type,
        Self::Preview,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Bind => "source.host",
            Self::All => "aql.query",
            Self::Exists => "aql.exists",
            Self::Wait => "aql.wait",
            Self::Click => "aql.click",
            Self::Type => "aql.type_text",
            Self::Preview => "aql.preview",
            Self::Keys => "aql.press_keys",
        }
    }
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
    let mut focused = false;
    let mut keys = Vec::new();
    let (platform, query, interval_ms, condition) = if matches!(kind, QueryKind::Keys) {
        let config: KeysTargetConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        if config.platform != TargetPlatform::Uia {
            return Err("按键输入目前只支持 UIA 平台".into());
        }
        keys = super::keys::parse(&config.keys)?;
        (config.platform, config.query, 0, WaitCondition::Exists)
    } else if matches!(kind, QueryKind::Wait) {
        let config: WaitTargetConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        if config.interval_ms < 10 || config.interval_ms > 60_000 {
            return Err("轮询间隔必须在 10..=60000 毫秒".into());
        }
        (
            config.platform,
            config.query,
            config.interval_ms,
            config.condition,
        )
    } else if matches!(kind, QueryKind::Type) {
        let config: TypeTargetConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        // 仅接受底层已明确保证的编辑语义，不用覆盖或盲输模拟其他模式。
        if !matches!(
            (config.platform, config.mode),
            (TargetPlatform::Cdp, TextEntryMode::Append)
                | (TargetPlatform::Uia, TextEntryMode::Selection)
                | (TargetPlatform::Uia, TextEntryMode::FocusedSelection)
        ) {
            return Err("此平台尚不支持所选输入方式；CDP 支持追加，UIA 支持当前选区输入，OCR 输入需先实现焦点确认".into());
        }
        focused = matches!(config.mode, TextEntryMode::FocusedSelection);
        (config.platform, config.query, 0, WaitCondition::Exists)
    } else {
        let config: TargetConfig =
            serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        (config.platform, config.query, 0, WaitCondition::Exists)
    };
    let query = argusflow_aql::compile_target(&query).map_err(|e| e.to_string())?;
    if matches!(kind, QueryKind::Type) && query.parameters().contains_key("text") {
        return Err("text 为输入文字节点保留端口，AQL 参数请使用其他名称".into());
    }
    Ok(Arc::new(QueryTask {
        kind,
        query,
        interval_ms,
        condition,
        platform,
        host,
        focused,
        keys,
    }))
}
