use serde::{Deserialize, Serialize};

use crate::EditorRange;

/// Lossless lexer 的 token 类别；token 文本始终从原始 source range 读取。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawTokenKind {
    /// 标识符、属性、函数或关键字。
    Identifier,
    /// 双引号字符串，包括未结束字符串。
    String,
    /// `/.../i` 正则，包括未结束正则。
    Regex,
    /// 十进制整数。
    Integer,
    /// `true` 或 `false`。
    Boolean,
    /// 空白与换行。
    Whitespace,
    /// `(`。
    LeftParen,
    /// `)`。
    RightParen,
    /// `,`。
    Comma,
    /// `=`、`!=`、`>` 或 `>>`。
    Operator,
    /// 无法识别或不完整的 token。
    Error,
}

/// 同时保存 Rust 内部字节区间和 WebView UTF-16 区间的 lossless token。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawToken {
    /// 词法类别。
    pub kind: RawTokenKind,
    /// 原始 token 文本。
    pub text: String,
    /// WebView 协议范围。
    pub range: EditorRange,
    /// Rust 内部起始 UTF-8 字节偏移，不参与序列化。
    #[serde(skip)]
    pub(crate) byte_start: usize,
    /// Rust 内部结束 UTF-8 字节偏移，不参与序列化。
    #[serde(skip)]
    pub(crate) byte_end: usize,
}

/// Recovery CST 中的节点类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CstNodeKind {
    /// 完整文档根节点。
    Document,
    /// 标识符与其括号参数组成的调用节点。
    Call,
    /// 无法闭合或包含错误 token 的恢复节点。
    Error,
}

/// CST 子元素，可以是嵌套节点或完整 token（包含 trivia）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CstElement {
    /// 嵌套语法节点。
    Node { node: CstNode },
    /// 原始词法 token。
    Token { token: RawToken },
}

/// 可在不完整文档上构造的具体语法节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstNode {
    /// 节点类别。
    pub kind: CstNodeKind,
    /// 节点覆盖的 WebView 范围。
    pub range: EditorRange,
    /// 包括 whitespace 在内的有序子元素。
    pub children: Vec<CstElement>,
}

/// Lossless token 与 Recovery CST 的组合结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxTree {
    /// Recovery CST 根节点。
    pub root: CstNode,
    /// 便于高亮与光标查询的平坦 lossless token 序列。
    pub tokens: Vec<RawToken>,
}
