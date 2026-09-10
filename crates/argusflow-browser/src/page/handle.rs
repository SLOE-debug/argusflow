//! 主文档页面与元素句柄，页面替换后不复用旧 DOM 身份。
use super::input;
use crate::BrowserError as Failure;
use crate::{
    Element,
    browser::validate_url,
    cdp::{Cleanup, Connection, PageState, UnknownResource},
};
use argusflow_core::{CssPoint, FailureKind, Key, Operation, OperationOptions, ScrollAxis};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::Ordering};

/// 普通页面 target 的只读描述。
#[derive(Debug, Clone, Deserialize)]
pub struct PageInfo {
    #[serde(rename = "targetId")]
    target_id: String,
    title: String,
    url: String,
}
impl PageInfo {
    /// 浏览器分配的 target 标识。
    pub fn target_id(&self) -> &str {
        &self.target_id
    }
    /// 页面标题。
    pub fn title(&self) -> &str {
        &self.title
    }
    /// 页面 URL。
    pub fn url(&self) -> &str {
        &self.url
    }
}
pub(crate) struct Inner {
    pub(crate) connection: Connection,
    pub(crate) session: String,
    pub(crate) state: Arc<PageState>,
    pub(crate) context: Option<String>,
    pub(crate) document: tokio::sync::Mutex<Option<(u64, i64)>>,
    pub(crate) aql_frames: tokio::sync::Mutex<
        std::collections::HashMap<String, std::sync::Weak<super::aql::FrameSession>>,
    >,
}
/// 已附加主文档的页面会话。
#[derive(Clone)]
pub struct Page {
    pub(crate) inner: Arc<Inner>,
}
impl Page {
    pub(crate) async fn attach(
        connection: Connection,
        target: &str,
        operation: &Operation,
    ) -> Result<Self, Failure> {
        let permit = connection
            .inner
            .resources
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::new(FailureKind::Busy, "page_attach", "会话清理额度已满"))?;
        let mut unknown = UnknownResource(Some(connection.clone()), operation.clone());
        let result = connection
            .command(
                None,
                "Target.attachToTarget",
                json!({"targetId":target,"flatten":true}),
                true,
                operation,
            )
            .await?;
        let session = result["sessionId"]
            .as_str()
            .ok_or_else(|| protocol("附加响应缺少 sessionId"))?
            .to_owned();
        let state = connection.register(session.clone(), target.to_owned());
        let mut cleanup = Cleanup::new(connection.clone(), None, permit);
        cleanup
            .commands
            .push(("Target.detachFromTarget", json!({"sessionId":session})));
        unknown.0.take();
        let page = Self {
            inner: Arc::new(Inner {
                connection,
                session,
                state,
                context: None,
                document: tokio::sync::Mutex::new(None),
                aql_frames: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            }),
        };
        for method in [
            "Runtime.enable",
            "Page.enable",
            "DOM.enable",
            "Inspector.enable",
        ] {
            let params = if method == "DOM.enable" {
                json!({"includeWhitespace":"all"})
            } else {
                json!({})
            };
            page.command(method, params, false, operation).await?;
        }
        cleanup.commands.clear();
        Ok(page)
    }
    /// 当前页面 target 标识。
    pub fn target_id(&self) -> &str {
        &self.inner.state.target
    }
    /// 页面与底层连接是否仍可用。
    pub fn is_open(&self) -> bool {
        self.inner
            .connection
            .inner
            .health
            .check(Some(&self.inner.session))
            .is_ok()
    }
    pub(crate) async fn command(
        &self,
        method: &'static str,
        params: Value,
        effect: bool,
        operation: &Operation,
    ) -> Result<Value, Failure> {
        self.inner
            .state
            .check()
            .map_err(|error| operation.contextualize(error))?;
        self.inner
            .connection
            .command(Some(&self.inner.session), method, params, effect, operation)
            .await
    }
    /// 导航到绝对 URL；等待当前主文档达到 interactive/complete，不等待网络空闲。
    pub async fn navigate(&self, url: &str, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        self.navigate_with_operation(url, &operation).await
    }
    /// 导航沿用调用方票据，包含等待文档提交的全部时间。
    pub async fn navigate_with_operation(
        &self,
        url: &str,
        operation: &Operation,
    ) -> Result<(), Failure> {
        validate_url(url)?;
        let mut guard = operation.cancel_on_drop();
        self.inner.state.epoch.fetch_add(1, Ordering::AcqRel);
        let result = self
            .command("Page.navigate", json!({"url":url}), true, operation)
            .await?;
        if result.get("errorText").is_some() {
            return Err(operation.contextualize(protocol("浏览器导航失败")));
        }
        // Page.navigate 响应可能早于新文档提交；先以 loaderId 确认主 frame。
        if let Some(loader) = result["loaderId"].as_str() {
            loop {
                let tree = self
                    .command("Page.getFrameTree", json!({}), false, operation)
                    .await?;
                if tree["frameTree"]["frame"]["loaderId"].as_str() == Some(loader) {
                    break;
                }
                operation.check("navigation_commit")?;
                tokio::time::sleep(
                    operation
                        .remaining()
                        .min(std::time::Duration::from_millis(25)),
                )
                .await;
            }
        }
        loop {
            operation.check("navigation_ready")?;
            let result = self
                .command(
                    "Runtime.evaluate",
                    json!({"expression":"document.readyState","returnByValue":true}),
                    false,
                    operation,
                )
                .await?;
            if matches!(
                result.pointer("/result/value").and_then(Value::as_str),
                Some("interactive" | "complete")
            ) {
                guard.disarm();
                return Ok(());
            }
            tokio::time::sleep(
                operation
                    .remaining()
                    .min(std::time::Duration::from_millis(25)),
            )
            .await;
        }
    }
    /// 仅在主文档查询 CSS；不穿透 iframe 或 Shadow DOM。
    pub async fn find_all(
        &self,
        selector: &str,
        options: OperationOptions,
    ) -> Result<Vec<Element>, Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        self.query(selector, false, &operation).await
    }
    /// 查询必须恰好匹配一个元素。
    pub async fn find_unique(
        &self,
        selector: &str,
        options: OperationOptions,
    ) -> Result<Element, Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let mut result = self.query(selector, true, &operation).await?;
        Ok(result.remove(0))
    }
    async fn query(
        &self,
        selector: &str,
        unique: bool,
        operation: &Operation,
    ) -> Result<Vec<Element>, Failure> {
        if selector.is_empty() || selector.len() > 16_384 {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "css_query",
                "CSS 选择器为空或过长",
            ));
        }
        let epoch = self.inner.state.epoch.load(Ordering::Acquire);
        let mut document = self.inner.document.try_lock().map_err(|_| {
            Failure::new(
                FailureKind::Busy,
                "css_query",
                "当前页面已有查询正在建立 DOM 身份",
            )
        })?;
        // DOM.getDocument 会重建前端 nodeId 绑定。同一代际必须复用根，
        // 否则后续查询会使之前交付的元素失效，却没有 documentUpdated 事件。
        let root = if let Some((generation, root)) = *document
            && generation == epoch
        {
            root
        } else {
            let result = self
                .command(
                    "DOM.getDocument",
                    json!({"depth":0,"pierce":false}),
                    false,
                    operation,
                )
                .await?;
            let root = result["root"]["nodeId"]
                .as_i64()
                .ok_or_else(|| protocol("主文档缺少 nodeId"))?;
            *document = Some((epoch, root));
            root
        };
        let result = self
            .command(
                "DOM.querySelectorAll",
                json!({"nodeId":root,"selector":selector}),
                false,
                operation,
            )
            .await?;
        let nodes = result["nodeIds"]
            .as_array()
            .ok_or_else(|| protocol("CSS 响应缺少 nodeIds"))?;
        if nodes.len() > self.inner.connection.inner.config.max_elements {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "css_query",
                "CSS 查询结果数量超限",
            ));
        }
        if unique && nodes.len() != 1 {
            return Err(Failure::new(
                if nodes.is_empty() {
                    FailureKind::NotFound
                } else {
                    FailureKind::Ambiguous
                },
                "css_query",
                "CSS 唯一查询没有恰好一个结果",
            ));
        }
        if epoch != self.inner.state.epoch.load(Ordering::Acquire) {
            return Err(stale());
        }
        nodes
            .iter()
            .map(|node| {
                Ok(Element {
                    page: Arc::downgrade(&self.inner),
                    epoch,
                    node: node
                        .as_i64()
                        .ok_or_else(|| protocol("元素 nodeId 不是整数"))?,
                })
            })
            .collect()
    }
    /// 显式执行调用方 JavaScript，结果按值返回；调用始终视为可能有副作用。
    pub async fn evaluate(
        &self,
        expression: &str,
        options: OperationOptions,
    ) -> Result<Value, Failure> {
        if expression.len() > self.inner.connection.inner.config.max_message_bytes / 2 {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "script",
                "脚本超过连接消息预算的一半",
            ));
        }
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let result=self.command("Runtime.evaluate",json!({"expression":expression,"returnByValue":true,"awaitPromise":true,"timeout":operation.remaining().as_millis() as u64}),true,&operation).await?;
        remote_value(result).map_err(|error| operation.contextualize(error))
    }
    /// 向主页面插入文字；焦点由调用者显式设置。
    pub async fn insert_text(&self, text: &str, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        input::text(self, text, &operation).await
    }
    /// 向页面发送组合键，取消后释放本次已按下的键。
    pub async fn press(&self, keys: &[Key], options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        input::keys(self, keys, &operation).await
    }
    /// 在主视口位置发送滚轮，delta 单位为 CSS 像素。
    pub async fn wheel(
        &self,
        point: CssPoint,
        axis: ScrollAxis,
        delta: f64,
        options: OperationOptions,
    ) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        input::wheel(self, point, axis, delta, &operation).await
    }
    /// 关闭当前页面，不关闭其他页面或浏览器。
    pub async fn close(&self, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        self.close_with_operation(&operation).await
    }
    /// 使用共享票据关闭页面并清理自有 context。
    pub async fn close_with_operation(&self, operation: &Operation) -> Result<(), Failure> {
        let mut guard = operation.cancel_on_drop();
        if !self.inner.state.closed.load(Ordering::Acquire) {
            self.inner
                .connection
                .command(
                    None,
                    "Target.closeTarget",
                    json!({"targetId":self.target_id()}),
                    true,
                    operation,
                )
                .await?;
            self.inner.state.closed.store(true, Ordering::Release);
        }
        if let Some(context) = &self.inner.context {
            self.inner
                .connection
                .command(
                    None,
                    "Target.disposeBrowserContext",
                    json!({"browserContextId":context}),
                    true,
                    operation,
                )
                .await?;
        }
        guard.disarm();
        Ok(())
    }
    /// 只脱离当前会话，页面继续存在。
    pub async fn detach(&self, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        self.detach_operation(&operation).await
    }
    /// 使用调用方票据仅脱离会话，保留外部页面。
    pub async fn detach_with_operation(&self, operation: &Operation) -> Result<(), Failure> {
        let mut guard = operation.cancel_on_drop();
        self.detach_operation(operation).await?;
        guard.disarm();
        Ok(())
    }
    pub(crate) async fn detach_operation(&self, operation: &Operation) -> Result<(), Failure> {
        self.inner
            .connection
            .command(
                None,
                "Target.detachFromTarget",
                json!({"sessionId":self.inner.session}),
                false,
                operation,
            )
            .await?;
        self.inner.state.closed.store(true, Ordering::Release);
        self.inner
            .connection
            .inner
            .health
            .pages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.inner.session);
        Ok(())
    }
}

pub(crate) fn remote_value(result: Value) -> Result<Value, Failure> {
    if result.get("exceptionDetails").is_some() {
        return Err(protocol("JavaScript 执行异常")
            .with_source(ScriptError(result["exceptionDetails"].clone())));
    }
    if result["result"]["type"].as_str() == Some("undefined") {
        return Ok(Value::Null);
    }
    result["result"]
        .get("value")
        .cloned()
        .ok_or_else(|| protocol("JavaScript 结果无法按 JSON 值返回"))
}
#[derive(Debug, thiserror::Error)]
#[error("JavaScript exception: {0}")]
struct ScriptError(Value);
pub(crate) fn protocol(message: &str) -> Failure {
    Failure::new(FailureKind::Protocol, "page", message)
}
pub(crate) fn stale() -> Failure {
    Failure::new(
        FailureKind::StaleHandle,
        "dom_element",
        "DOM 元素句柄已经失效",
    )
}
