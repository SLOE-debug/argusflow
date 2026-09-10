//! 编辑器根据稳定英文符号表提供补全，不维护另一套语法。
use super::documentation;
use crate::{Attribute, Role};
use serde::{Deserialize, Serialize};

/// 补全和悬浮符号类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// 元素角色。
    Role,
    /// 查询函数。
    Function,
    /// 属性。
    Attribute,
    /// 比较和逻辑操作。
    Operator,
    /// 布尔值。
    Boolean,
    /// 调用方绑定的命名参数。
    Parameter,
}
/// 无平台依赖的英文符号说明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// 规范英文标识。
    pub name: String,
    /// 编辑器分类。
    pub kind: SymbolKind,
    /// 中文说明。
    pub description: String,
    /// 可读用法或属性类型。
    pub signature: String,
    /// 可以放入查询中的用法示例。
    pub example: String,
}
/// 当前语法的唯一符号清单。
pub fn symbols() -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for role in Role::ALL {
        symbols.push(Symbol {
            name: role.name().into(),
            kind: SymbolKind::Role,
            description: documentation::role(*role).into(),
            signature: format!("{}(条件) → 元素集合", role.name()),
            example: format!("{}()", role.name()),
        });
    }
    for attribute in Attribute::ALL {
        symbols.push(Symbol {
            name: attribute.name().into(),
            kind: SymbolKind::Attribute,
            description: documentation::attribute(*attribute).into(),
            signature: format!(
                "{}: {}",
                attribute.name(),
                documentation::value_type(attribute.value_type())
            ),
            example: documentation::attribute_example(*attribute),
        });
    }
    for (name, description, signature, example) in [
        (
            "first",
            "选择来源顺序中的第一个匹配；没有匹配时返回空结果。UIA/DOM 使用树先序，OCR 使用阅读顺序。",
            "first(查询) → 至多一个元素",
            "first(button(name = \"保存\"))",
        ),
        (
            "nth",
            "选择第几个匹配。序号是从 1 开始的整数；超出匹配数量时返回空结果。",
            "nth(查询, 序号) → 至多一个元素",
            "nth(button(), 2)",
        ),
        (
            "css",
            "使用浏览器原生 CSS 选择器。仅支持浏览器来源；不会自动进入 iframe 或 Shadow Root。",
            "css(\"选择器\") → 元素集合",
            "css(\".save-button\")",
        ),
        (
            "frame",
            "先唯一定位 iframe 宿主，再进入它的文档。支持跨域和嵌套；必须继续用 > 或 >> 指定内部目标。",
            "frame(宿主查询) >> 内部查询",
            "frame(css(\"iframe\")) >> textbox()",
        ),
        (
            "shadow",
            "先唯一定位宿主，再进入开放的 Shadow Root。必须继续指定内部目标；不支持封闭的 Shadow Root。",
            "shadow(宿主查询) >> 内部查询",
            "shadow(css(\"settings-panel\")) >> button()",
        ),
    ] {
        symbols.push(Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
            description: description.into(),
            signature: signature.into(),
            example: example.into(),
        });
    }
    for (name, description, signature, example) in [
        (
            "contains",
            "检查文本中是否包含指定内容，区分大小写。仅用于文本属性。",
            "文本属性 contains 文本",
            "name contains \"保存\"",
        ),
        (
            "starts_with",
            "检查文本是否以指定内容开头，区分大小写。",
            "文本属性 starts_with 文本",
            "name starts_with \"设置\"",
        ),
        (
            "ends_with",
            "检查文本是否以指定内容结尾，区分大小写。",
            "文本属性 ends_with 文本",
            "name ends_with \"文件\"",
        ),
        (
            "matches",
            "使用 Rust 正则匹配文本。可加 i 标志忽略大小写；不支持环视或回溯引用。",
            "文本属性 matches /模式/i",
            "name matches /保存.*/i",
        ),
        (
            "and",
            "两侧条件都成立才匹配，与逗号等价。优先级低于 not、高于 or；可以用括号分组。",
            "条件 and 条件",
            "name = \"保存\" and enabled = true",
        ),
        (
            "or",
            "两侧条件至少一个成立就匹配。优先级低于 not 和 and。",
            "条件 or 条件",
            "name = \"保存\" or name = \"提交\"",
        ),
        (
            "not",
            "对条件取反，优先级最高。属性缺失时仍不能判定为匹配。",
            "not 条件",
            "not (enabled = false)",
        ),
    ] {
        symbols.push(Symbol {
            name: name.into(),
            kind: SymbolKind::Operator,
            description: description.into(),
            signature: signature.into(),
            example: example.into(),
        });
    }
    for name in ["true", "false"] {
        symbols.push(Symbol {
            name: name.into(),
            kind: SymbolKind::Boolean,
            description: "布尔状态字面量".into(),
            signature: format!("{name}: 布尔"),
            example: format!("enabled = {name}"),
        });
    }
    symbols
}
