//! 动作前重新校验 frame 身份和命中；不执行脚本点击或重试。
use super::super::{handle::protocol, input};
use super::{
    geometry::{FrameMetrics, project},
    model::BrowserMatch,
};
use crate::BrowserError;
use argusflow_core::{ClickCount, CssPoint, FailureKind, MouseButton, Operation};
use serde_json::json;

impl BrowserMatch {
    /// 对当前身份执行一次 CDP 左键单击，定位方负责每次重新查询。
    pub async fn click_aql(&self, operation: &Operation) -> Result<(), BrowserError> {
        let mut guard = operation.cancel_on_drop();
        self.context.check()?;
        let top = self.context.top_page();
        let permit = input::reserve(top)?;
        let mut owners = Vec::new();
        let mut context = &self.context;
        while let Some(owner) = &context.owner {
            owners.push(owner);
            context = &owner.context;
        }
        for owner in owners.iter().rev() {
            owner.scroll(operation).await?;
        }
        self.scroll(operation).await?;
        let value = self
            .context
            .call(
                self.node.backend_node_id,
                include_str!("action.js"),
                vec![json!("point"), json!(null)],
                false,
                operation,
            )
            .await?;
        let coordinates: [f64; 2] = serde_json::from_value(value).map_err(|_| {
            BrowserError::new(
                FailureKind::InvalidInput,
                "aql_click",
                "目标没有可点击且未遮挡的位置",
            )
        })?;
        let mut point = CssPoint::new(coordinates[0], coordinates[1])?;
        for owner in owners {
            let metrics = owner
                .context
                .call(
                    owner.node.backend_node_id,
                    include_str!("action.js"),
                    vec![json!("frame"), json!(null)],
                    false,
                    operation,
                )
                .await?;
            let metrics: FrameMetrics = serde_json::from_value(metrics)
                .map_err(|e| protocol("iframe 几何响应格式错误").with_source(e))?;
            point = project(point, &metrics)?;
            let hit = owner
                .context
                .call(
                    owner.node.backend_node_id,
                    include_str!("action.js"),
                    vec![json!("hit"), json!([point.x(), point.y()])],
                    false,
                    operation,
                )
                .await?;
            if hit != true {
                return Err(BrowserError::new(
                    FailureKind::InvalidInput,
                    "aql_click",
                    "iframe 点击位置被其他元素遮挡",
                ));
            }
        }
        self.context.check()?;
        input::click(
            top,
            point,
            MouseButton::Left,
            ClickCount::Single,
            operation,
            permit,
        )
        .await?;
        guard.disarm();
        Ok(())
    }
    /// 聚焦并将已有选区收起到末尾后插入，不清空或替换原内容。
    pub async fn type_text_aql(
        &self,
        text: &str,
        operation: &Operation,
    ) -> Result<(), BrowserError> {
        if text.is_empty() || text.len() > 16_384 {
            return Err(BrowserError::new(
                FailureKind::InvalidInput,
                "aql_text",
                "输入文字必须为 1 到 16384 字节",
            ));
        }
        let mut guard = operation.cancel_on_drop();
        let _permit = input::reserve(self.context.top_page())?;
        let focused = self
            .context
            .call(
                self.node.backend_node_id,
                include_str!("action.js"),
                vec![json!("focus"), json!(null)],
                true,
                operation,
            )
            .await?;
        if focused != true {
            return Err(BrowserError::new(
                FailureKind::Unsupported,
                "aql_text",
                "目标不是可编辑且可聚焦的输入元素",
            ));
        }
        self.context.check()?;
        self.context
            .page
            .command("Input.insertText", json!({"text":text}), true, operation)
            .await?;
        guard.disarm();
        Ok(())
    }
    async fn scroll(&self, operation: &Operation) -> Result<(), BrowserError> {
        self.context.check()?;
        self.context
            .page
            .command(
                "DOM.scrollIntoViewIfNeeded",
                json!({"backendNodeId":self.node.backend_node_id}),
                true,
                operation,
            )
            .await?;
        self.context.check()
    }
}
