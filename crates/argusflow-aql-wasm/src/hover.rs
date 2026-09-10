//! 英文符号说明映射为中文显示，参数名及源码范围保持原样。
use crate::{localization::chinese, localize, translate};
use argusflow_aql::{EditorPosition, EditorRange, Span, SymbolKind};
use serde::Serialize;

/// 编辑器悬浮显示的符号类别、用法和示例。
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedHover {
    /// 中文与英文符号对照，参数名不翻译。
    pub title: String,
    /// 角色、函数、属性、运算符、布尔值或参数。
    pub kind: SymbolKind,
    /// 参数及返回值，或属性/参数类型。
    pub signature: String,
    /// 中文用途与约束。
    pub description: String,
    /// 中文用法示例。
    pub example: String,
    /// 中文原稿中的 UTF-16 范围。
    pub range: EditorRange,
}
/// 在中文或英文编辑文本上获取说明，不把字面量解释为符号。
pub fn hover(source: &str, position: EditorPosition) -> Option<LocalizedHover> {
    let translated = translate(source);
    let offset = position.offset(source);
    let generated = translated.to_generated(Span::new(offset, offset));
    let item = argusflow_aql::hover(
        translated.source(),
        EditorPosition::at(translated.source(), generated.start),
    )?;
    let symbol = item.symbol;
    Some(LocalizedHover {
        title: if symbol.kind == SymbolKind::Parameter {
            symbol.name.clone()
        } else {
            format!("{} · {}", chinese(&symbol.name), symbol.name)
        },
        kind: symbol.kind,
        signature: localize(&symbol.signature).source().into(),
        description: symbol.description,
        example: localize(&symbol.example).source().into(),
        range: translated.to_original(item.span).editor_range(source),
    })
}
