//! 根据重新定位的元素与全文前置条件重建 UTF-16 选区，再发送真实复制键。
use super::model::BrowserMatch;
use crate::BrowserError;
use argusflow_core::{FailureKind, Key, Operation};
use serde_json::json;

impl BrowserMatch {
    /// 语义重放选区并复制，不重放像素轨迹；剪贴板结果由调用方独立验证。
    pub async fn copy_text_range(
        &self,
        expected: &str,
        start: u32,
        end: u32,
        operation: &Operation,
    ) -> Result<String, BrowserError> {
        if expected.len() > 16384 || start >= end || end as usize > expected.encode_utf16().count()
        {
            return Err(BrowserError::new(
                FailureKind::InvalidInput,
                "copy_range",
                "文本或范围无效",
            ));
        }
        self.context.check()?;
        let mut guard = operation.cancel_on_drop();
        self.context
            .top_page()
            .command("Page.bringToFront", json!({}), true, operation)
            .await?;
        let selected = self
            .context
            .call(
                self.node.backend_node_id,
                include_str!("text_selection.js"),
                vec![json!(expected), json!(start), json!(end)],
                true,
                operation,
            )
            .await?;
        let selected = selected
            .as_str()
            .ok_or_else(|| BrowserError::new(FailureKind::Protocol, "copy_range", "选区响应无效"))?
            .to_owned();
        self.context.check()?;
        super::super::input::keys(
            &self.context.page,
            &[Key::Control, Key::Letter('C')],
            operation,
        )
        .await?;
        guard.disarm();
        Ok(selected)
    }
}
