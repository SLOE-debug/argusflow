//! 显式页面的被动事件观察；不点击、不聚焦、不滚动。
use crate::{BrowserError, Page};
use argusflow_core::{Operation, OperationOptions};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::Ordering;
/// 页面观察结果，仅限主 frame 和开放 Shadow DOM。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageObservation {
    /// 附加页面身份。
    pub target: String,
    /// 当前主 frame。
    pub frame: Option<String>,
    /// 文档代际；导航、断连和上下文销毁使旧身份失效。
    pub epoch: u64,
    /// 页面 performance.now，毫秒。
    pub page_now: f64,
    /// 文档是否拥有焦点。
    pub focused: bool,
    /// 监听队列溢出数量。
    pub lost: u32,
    /// 最近实际事件，不生成额外系统操作。
    pub events: Vec<PageEvent>,
    /// 当前焦点属性。
    pub active: Option<PageTarget>,
}
/// 普通 DOM 属性，不保留 JS 对象。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTarget {
    /// 标签。
    pub tag: String,
    /// ARIA 角色。
    pub role: String,
    /// 标签／可见文字。
    pub name: String,
    /// DOM id。
    pub id: String,
    /// 输入类型。
    pub input_type: String,
    /// 实际编辑值；密码为空。
    pub value: Option<String>,
    /// 是否密码。
    pub password: bool,
    /// 视口 CSS 边界。
    pub bounds: [f64; 4],
    /// 开放 Shadow 根和祖先路径，最多四层。
    pub context: Vec<String>,
    /// 到达边界或预算。
    pub truncated: bool,
}
/// 原生页面事件事实，不以键码拼接字符串。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEvent {
    /// 当前文档内部事件序号。
    pub sequence: u64,
    /// 事件类别。
    pub kind: String,
    /// performance.now 时间，需由宿主请求区间换算。
    pub time: f64,
    /// 浏览器可信事件标记。
    pub trusted: bool,
    /// inputType，区分 IME／粘贴／删除／替换。
    pub input_type: String,
    /// 目标属性，密码值不读取。
    pub target: Option<PageTarget>,
}
/// 一次安装的监听所有者，stop 必须在暂停／停止时调用。
pub struct PageObserver {
    page: Page,
    script: String,
}
impl PageObserver {
    /// 显式绑定的页面才安装，唯一命名空间防止重复安装。
    pub async fn install(page: Page, options: OperationOptions) -> Result<Self, BrowserError> {
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let response = page
            .command(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({"source":include_str!("observation.js")}),
                false,
                &operation,
            )
            .await?;
        let script = response["identifier"]
            .as_str()
            .ok_or_else(|| super::handle::protocol("监听安装缺少标识"))?
            .to_owned();
        let observer = Self { page, script };
        if let Err(error) = observer
            .page
            .command(
                "Runtime.evaluate",
                json!({"expression":include_str!("observation.js"),"returnByValue":true}),
                false,
                &operation,
            )
            .await
        {
            let _ = observer.stop(OperationOptions::default()).await;
            return Err(error);
        }
        Ok(observer)
    }
    /// 获取有界队列；主机请求区间与 page_now 给出时间转换的不确定区间。
    pub async fn observe(
        &self,
        options: OperationOptions,
    ) -> Result<PageObservation, BrowserError> {
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let epoch = self.page.inner.state.epoch.load(Ordering::Acquire);
        let result=self.page.command("Runtime.evaluate",json!({"expression":"globalThis.__argusflowRecorderV1?.take()","returnByValue":true}),false,&operation).await?;
        let value = super::handle::remote_value(result)?;
        #[derive(Deserialize)]
        struct Snapshot {
            page_now: f64,
            focused: bool,
            lost: u32,
            events: Vec<PageEvent>,
            active: Option<PageTarget>,
        }
        let snapshot: Snapshot = serde_json::from_value(value)
            .map_err(|e| super::handle::protocol("页面监听尚未安装或格式无效").with_source(e))?;
        if epoch != self.page.inner.state.epoch.load(Ordering::Acquire) || !self.page.is_open() {
            return Err(super::handle::stale());
        }
        Ok(PageObservation {
            target: self.page.target_id().into(),
            frame: self
                .page
                .inner
                .state
                .main_frame
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
            epoch,
            page_now: snapshot.page_now,
            focused: snapshot.focused,
            lost: snapshot.lost,
            events: snapshot.events,
            active: snapshot.active,
        })
    }
    /// 卸载当前文档及后续导航脚本；只脱离时页面仍保留。
    pub async fn stop(&self, options: OperationOptions) -> Result<(), BrowserError> {
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let current=self.page.command("Runtime.evaluate",json!({"expression":"globalThis.__argusflowRecorderV1?.stop()","returnByValue":true}),false,&operation).await;
        let future = self
            .page
            .command(
                "Page.removeScriptToEvaluateOnNewDocument",
                json!({"identifier":self.script}),
                false,
                &operation,
            )
            .await;
        current?;
        future?;
        Ok(())
    }
}
