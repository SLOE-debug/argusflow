//! 只读文本观察，范围偏移按 UIA 返回字符串的 UTF-16 单元计量。
use serde::{Deserialize, Serialize};

/// TextPattern 能力与读取结果；不支持和空选区不是同一状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiaTextObservation {
    /// 控件没有提供 TextPattern。
    Unsupported,
    /// 密码控件，不读取正文或选区。
    Sensitive,
    /// 提供者调用失败，本次没有可靠的文本事实。
    Unavailable(String),
    /// 有界只读快照。
    Available(UiaTextSnapshot),
}

/// 同一次观察内读取的文档和选区，不承诺提供者支持原子快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiaTextSnapshot {
    /// 文档前缀，最多 8192 UTF-16 单元。
    pub document: String,
    /// 文档是否被预算截断。
    pub document_truncated: bool,
    /// ValuePattern 提供的可编辑性；未知时为 None，不能凭类名猜测。
    pub editable: Option<bool>,
    /// 实际选区；退化范围表示光标，空列表表示提供者没有返回范围。
    pub selections: Vec<UiaTextRange>,
    /// 超过范围数量或字符串预算。
    pub truncated: bool,
}

/// 一条原生文本范围，不使用所选文字搜索位置，因而允许重复正文。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiaTextRange {
    /// 是否为插入光标。
    pub collapsed: bool,
    /// 选中文字前缀，最多 8192 UTF-16 单元。
    pub text: String,
    /// 文字被预算截断。
    pub truncated: bool,
    /// 从文档开头到选区起点的 UTF-16 长度；前缀超预算则未知。
    pub start_utf16: Option<u32>,
    /// 排他结束偏移；选区全文或起点未知时为空。
    pub end_utf16: Option<u32>,
}
