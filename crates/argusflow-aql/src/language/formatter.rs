//! 仅调整空白，保留字面量、注释、条件顺序和参数。
use crate::{AqlError, TokenKind, compile, tokenize};

/// 对合法英文 AQL 格式化；不重排或删除任何非空白 token。
pub fn format_query(source: &str) -> Result<String, AqlError> {
    compile(source)?;
    let lexed = tokenize(source);
    let mut output = String::new();
    let mut previous = String::new();
    for token in lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Whitespace)
    {
        let text = token.text(source);
        if token.kind == TokenKind::Comment {
            if !output.is_empty() && !output.ends_with([' ', '\n']) {
                output.push(' ');
            }
            output.push_str(text);
            if text.starts_with("//") {
                output.push('\n');
            } else {
                output.push(' ');
            }
            previous.clear();
            continue;
        }
        let adjacent = matches!(text, ")" | ",")
            || previous == "("
            || (text == "("
                && (previous.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                    && !["and", "or", "not"].contains(&previous.as_str())));
        if !output.is_empty() && !output.ends_with([' ', '\n']) && !adjacent {
            output.push(' ');
        }
        output.push_str(text);
        previous = text.to_owned();
    }
    Ok(output.trim().to_owned())
}
