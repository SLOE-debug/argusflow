//! 系统剪贴板的只读事实；序号变化不等于某次按键导致变化。
use serde::{Deserialize, Serialize};

/// 读取状态，不把非文本格式或未读取当作空字符串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardContent {
    /// 相同序号，未重复读取内容。
    Unchanged,
    /// 当前没有 Unicode 文本格式。
    NoText,
    /// 实际文本及预算状态；空字符串也是有效文本。
    Text {
        /// Unicode 正文。
        text: String,
        /// 达到长度预算，仅保存前缀。
        truncated: bool,
    },
}

/// 一次成功的稳定序号读取；系统序号仅在当前会话内比较相等，不推断顺序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardObservation {
    /// 调用方上次成功读取序号；None 表示建立基线。
    pub previous_sequence: Option<u32>,
    /// 本次系统序号。
    pub sequence: u32,
    /// 读取内容。
    pub content: ClipboardContent,
}
