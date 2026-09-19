//! 比较同一控件的两次文本事实，不从差异推断用户意图。
use super::UiaTextObservation;
use crate::UiaObservation;
use serde::{Deserialize, Serialize};

/// 前后观察的可比较性与变化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiaTextChange {
    /// 首次观察或控件身份不同，不跨控件比较选区。
    Baseline,
    /// 任一端不支持、敏感、失败或截断，不能断言未变化。
    Unresolved,
    /// 相同控件两次完整观察之间的差异，不表示两次之间没有其他变化。
    Compared {
        /// 文档内容发生变化。
        document_changed: bool,
        /// 所选文字、端点或范围数量发生变化。
        selection_changed: bool,
    },
}
impl UiaObservation {
    /// 比较前一次观察；只保存事实，不读取屏幕、不变更焦点。
    pub fn text_change_from(&self, previous: Option<&Self>) -> UiaTextChange {
        let Some(previous) = previous else {
            return UiaTextChange::Baseline;
        };
        if self.runtime_id.is_empty()
            || self.runtime_id != previous.runtime_id
            || self.target.pid != previous.target.pid
        {
            return UiaTextChange::Baseline;
        }
        match (&previous.text, &self.text) {
            (UiaTextObservation::Available(before), UiaTextObservation::Available(after))
                if !before.truncated
                    && !after.truncated
                    && !before.document_truncated
                    && !after.document_truncated =>
            {
                UiaTextChange::Compared {
                    document_changed: before.document != after.document,
                    selection_changed: before.selections != after.selections,
                }
            }
            _ => UiaTextChange::Unresolved,
        }
    }
}
