//! 工作流的中文目标语言入口；平台专有选择器留在原生 API，不进入节点查询。
use super::{Translation, english, translate};
use crate::{AqlError, CompiledQuery, DiagnosticCode, TokenKind, compile, tokenize};

/// 中文节点查询允许使用的通用符号；与编辑器候选共用。
pub fn is_target_symbol(word: &str) -> bool {
    !word.contains('.') && !matches!(word, "key" | "css" | "frame" | "shadow")
}

/// 校验中文词汇并转换为共享的查询语法，保留诊断位置和参数原文。
pub fn translate_target(source: &str) -> Result<Translation, AqlError> {
    let lexed = tokenize(source);
    if let Some(error) = lexed.diagnostics.into_iter().next() {
        return Err(error);
    }
    for token in lexed.tokens {
        if token.kind != TokenKind::Identifier {
            continue;
        }
        let word = token.text(source);
        if word
            .strip_prefix('第')
            .and_then(|w| w.strip_suffix('个'))
            .is_some_and(|n| n.parse::<usize>().is_ok())
        {
            continue;
        }
        let base = word
            .strip_suffix("包含")
            .filter(|base| !base.is_empty())
            .unwrap_or(word);
        let translated = english(base);
        if translated == base || !is_target_symbol(translated) {
            return Err(AqlError::new(
                DiagnosticCode::UnknownSymbol,
                token.span,
                "请使用中文通用目标、属性和运算符；平台设置请在节点中配置",
            ));
        }
    }
    Ok(translate(source))
}

/// 编译节点中的中文 AQL，错误范围指向原始中文文本。
pub fn compile_target(source: &str) -> Result<CompiledQuery, AqlError> {
    let translated = translate_target(source)?;
    compile(translated.source()).map_err(|mut error| {
        error.span = translated.to_original(error.span);
        error
    })
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-aql/unit/target.rs"]
mod tests;
