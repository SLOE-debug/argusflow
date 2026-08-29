use serde::{Deserialize, Serialize};

use super::EditorRange;

/// AQL 编辑器可以直接渲染的语法类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxTokenKind {
    /// 元素语义角色。
    Role,
    /// 查询组合器或 escape hatch。
    Function,
    /// portable 属性名。
    Property,
    /// 后端专用命名空间前缀。
    Namespace,
    /// 比较或关系运算符。
    Operator,
    /// 字符串字面量。
    String,
    /// 正则字面量。
    Regex,
    /// 布尔字面量。
    Boolean,
    /// 整数字面量。
    Integer,
    /// `$name` 运行时参数引用。
    Parameter,
    /// 括号、逗号和点号。
    Punctuation,
    /// 空白与换行 trivia。
    Trivia,
    /// 尚无法归类或包含错误的源码片段。
    Unknown,
}

/// 一个不暴露 UTF-8 字节偏移的编辑器语法 token。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxToken {
    /// 由 Rust 语言引擎判定的类别。
    pub kind: SyntaxTokenKind,
    /// 浏览器安全的 UTF-16 文本范围。
    pub range: EditorRange,
}

/// 补全项的展示与插入类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionItemKind {
    /// 元素角色调用。
    Role,
    /// 查询函数。
    Function,
    /// 属性名。
    Property,
    /// 比较运算符。
    Operator,
    /// 常量值。
    Value,
}

/// 由 Rust 语言服务生成的单个补全候选。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    /// 补全列表中显示的稳定标签。
    pub label: String,
    /// 应替换的源码范围。
    pub replacement_range: EditorRange,
    /// 应写入文档的文本。
    pub insert_text: String,
    /// 候选类别。
    pub kind: CompletionItemKind,
    /// 可选的简短说明，由 UI 决定最终展示方式。
    pub detail: Option<String>,
}

/// 光标位置对应的语言说明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hover {
    /// 被说明的源码范围。
    pub range: EditorRange,
    /// 稳定的语言符号名。
    pub symbol: String,
    /// 供产品层本地化的说明代码。
    pub description_code: String,
}

/// 编辑器可以原子应用的文本修改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    /// 被替换的 UTF-16 范围。
    pub range: EditorRange,
    /// 新文本。
    pub new_text: String,
}
