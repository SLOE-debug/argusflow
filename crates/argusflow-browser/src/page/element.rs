//! 主文档元素身份、属性读取和控件输入。
use super::{
    handle::{Inner, protocol, remote_value, stale},
    input,
};
use crate::BrowserError as Failure;
use crate::{Page, cdp::Cleanup};
use argusflow_core::{ClickCount, CssPoint, FailureKind, MouseButton, Operation, OperationOptions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Weak, atomic::Ordering};

/// 绑定当前文档代际和页面的元素身份；不长期持有 JS RemoteObject。
#[derive(Clone)]
pub struct Element {
    pub(crate) page: Weak<Inner>,
    pub(crate) epoch: u64,
    pub(crate) node: i64,
}
/// 元素的普通 Rust 属性快照。
#[derive(Debug, Clone, Deserialize)]
pub struct ElementSnapshot {
    /// textContent，不包含布局推断。
    pub text: String,
    /// 当前 DOM 属性键值。
    pub attributes: std::collections::BTreeMap<String, String>,
    /// 主视口 CSS 像素边界：[x,y,width,height]。
    pub bounds: [f64; 4],
}
impl Element {
    fn page(&self) -> Result<Page, Failure> {
        let inner = self.page.upgrade().ok_or_else(stale)?;
        inner.state.check()?;
        if inner.state.epoch.load(Ordering::Acquire) != self.epoch {
            return Err(stale());
        }
        Ok(Page { inner })
    }
    /// 读取元素文字、属性和视口边界。
    pub async fn read(&self, options: OperationOptions) -> Result<ElementSnapshot, Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let value = self.call("function(){if(!this.isConnected)throw new Error('detached');const r=this.getBoundingClientRect();return {text:this.textContent||'',attributes:Object.fromEntries(Array.from(this.attributes,a=>[a.name,a.value])),bounds:[r.x,r.y,r.width,r.height]};}", &operation).await?;
        serde_json::from_value(value)
            .map_err(|error| protocol("元素快照格式错误").with_source(error))
    }
    /// 显式设置 DOM 焦点。
    pub async fn focus(&self, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let page = self.page()?;
        let _input = input::reserve(&page)?;
        page.command("DOM.focus", json!({"nodeId":self.node}), true, &operation)
            .await?;
        Ok(())
    }
    /// 显式滚动到可见区域。
    pub async fn scroll_into_view(&self, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        self.scroll(&operation).await
    }
    async fn scroll(&self, operation: &Operation) -> Result<(), Failure> {
        self.page()?
            .command(
                "DOM.scrollIntoViewIfNeeded",
                json!({"nodeId":self.node}),
                true,
                operation,
            )
            .await?;
        Ok(())
    }
    /// 在元素可见中心点发送 CDP 鼠标事件，先检查命中目标。
    pub async fn click(
        &self,
        button: MouseButton,
        count: ClickCount,
        options: OperationOptions,
    ) -> Result<(), Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let page = self.page()?;
        let permit = input::reserve(&page)?;
        let result = async {
            self.scroll(&operation).await?;
            let value = self.call("function(){if(!this.isConnected)throw new Error('detached');const r=this.getBoundingClientRect();const l=Math.max(0,r.left),t=Math.max(0,r.top),rr=Math.min(innerWidth,r.right),b=Math.min(innerHeight,r.bottom);const x=(l+rr)/2,y=(t+b)/2;const hit=document.elementFromPoint(x,y);if(l>=rr||t>=b||!(hit===this||this.contains(hit)))throw new Error('not clickable');return [x,y];}", &operation).await?;
            let [x,y]: [f64; 2] = serde_json::from_value(value).map_err(|error| protocol("点击点格式错误").with_source(error))?;
            input::click(&page, CssPoint::new(x,y)?, button, count, &operation, permit).await
        }.await;
        result.map_err(|error| operation.contextualize(error))
    }
    async fn call(&self, function: &str, operation: &Operation) -> Result<Value, Failure> {
        let page = self.page()?;
        let connection = &page.inner.connection;
        let permit = connection
            .inner
            .resources
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::new(FailureKind::Busy, "dom_object", "远端对象清理额度已满"))?;
        // 已知的 objectGroup 在 resolve 之前注册清理，响应丢失也能释放。
        let group = format!("argusflow-{}", operation.id());
        let mut cleanup =
            Cleanup::new(connection.clone(), Some(page.inner.session.clone()), permit);
        cleanup
            .commands
            .push(("Runtime.releaseObjectGroup", json!({"objectGroup":group})));
        let result = page
            .command(
                "DOM.resolveNode",
                json!({"nodeId":self.node,"objectGroup":group}),
                false,
                operation,
            )
            .await?;
        let object = result["object"]["objectId"].as_str().ok_or_else(stale)?;
        self.page()?;
        let result = page.command("Runtime.callFunctionOn", json!({"objectId":object,"functionDeclaration":function,"returnByValue":true,"silent":true}), false, operation).await?;
        self.page()?;
        remote_value(result)
    }
}
