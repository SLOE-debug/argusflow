//! 用英文词法位置选择符号；参数类型只采用编译器确认的结果。
use super::{Symbol, SymbolKind, documentation, symbols};
use crate::{EditorPosition, Span, TokenKind, ValueType, compile, tokenize};

/// 当前源码位置对应的语言说明。
#[derive(Debug, Clone)]
pub struct Hover {
    /// 角色、函数、属性或命名参数的说明。
    pub symbol: Symbol,
    /// 英文源码 UTF-8 范围。
    pub span: Span,
}
/// 支持有效源码和未完成草稿，不在字符串、正则或注释中解释关键字。
pub fn hover(source: &str, position: EditorPosition) -> Option<Hover> {
    let offset = position.offset(source);
    let lexed = tokenize(source);
    let (index, token) = lexed
        .tokens
        .iter()
        .enumerate()
        .find(|(_, token)| token.span.start <= offset && offset < token.span.end)?;
    let name = token.text(source);
    let symbol = match token.kind {
        TokenKind::Parameter => parameter(source, name),
        TokenKind::Identifier => {
            let next = lexed.tokens[index + 1..]
                .iter()
                .find(|t| !matches!(t.kind, TokenKind::Whitespace | TokenKind::Comment));
            let call = next.is_some_and(|t| t.text(source) == "(");
            symbols().into_iter().find(|symbol| {
                symbol.name == name
                    && match symbol.kind {
                        SymbolKind::Role | SymbolKind::Function => call,
                        SymbolKind::Attribute => !call,
                        _ => true,
                    }
            })?
        }
        _ => return None,
    };
    Some(Hover {
        symbol,
        span: token.span,
    })
}
fn parameter(source: &str, name: &str) -> Symbol {
    let kind = compile(source)
        .ok()
        .and_then(|query| query.parameters().get(&name[1..]).copied());
    let signature = format!(
        "{name}: {}",
        kind.map(documentation::value_type).unwrap_or("类型待确定")
    );
    let description = if kind.is_some() {
        "调用方通过类型化参数绑定提供值，执行时不会把值拼接到源码中。参数名区分大小写，中英文转换时保持原样。"
    } else {
        "命名参数由调用方通过类型化绑定传入。当前草稿尚未通过检查，修正语法或类型冲突后可确认参数类型；参数名保持原样。"
    };
    let attribute = match kind {
        Some(ValueType::Boolean) => "enabled",
        Some(ValueType::Number) => "confidence",
        _ => "name",
    };
    Symbol {
        name: name.into(),
        kind: SymbolKind::Parameter,
        description: description.into(),
        signature,
        example: format!("{attribute} = {name}"),
    }
}
