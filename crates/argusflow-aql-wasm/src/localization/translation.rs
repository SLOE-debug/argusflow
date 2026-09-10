//! 无损 token 转换以及不同字节长度之间的双向位置映射。
use super::{chinese, english};
use argusflow_aql::{Span, TokenKind, tokenize};

#[derive(Debug, Clone)]
struct Segment {
    original: Span,
    generated: Span,
}
/// 转换后的文本及其到编辑草稿的精确边界映射。
#[derive(Debug, Clone)]
pub struct Translation {
    source: String,
    segments: Vec<Segment>,
    original_length: usize,
}
impl Translation {
    /// 转换文本，不改变字面量和注释。
    pub fn source(&self) -> &str {
        &self.source
    }
    /// 把英文诊断或替换范围映射回中文编辑草稿。
    pub fn to_original(&self, span: Span) -> Span {
        Span::new(
            self.map(span.start, false, false),
            self.map(span.end, true, false),
        )
    }
    /// 把中文编辑范围映射到英文源码。
    pub fn to_generated(&self, span: Span) -> Span {
        Span::new(
            self.map(span.start, false, true),
            self.map(span.end, true, true),
        )
    }
    fn map(&self, offset: usize, end: bool, forward: bool) -> usize {
        for segment in &self.segments {
            let (from, to) = if forward {
                (segment.original, segment.generated)
            } else {
                (segment.generated, segment.original)
            };
            if offset >= from.start && offset <= from.end {
                if offset == from.start {
                    return to.start;
                }
                if offset == from.end {
                    return to.end;
                }
                if from.end - from.start == to.end - to.start {
                    return to.start + offset - from.start;
                }
                return if end { to.end } else { to.start };
            }
        }
        if forward {
            self.source.len()
        } else {
            self.original_length
        }
    }
}
/// 将中文关键字转换为英文；非关键字 token 一字不改。
pub fn translate(source: &str) -> Translation {
    convert(source, english)
}
/// 将英文关键字转换为中文，用于导入和格式化回写。
pub fn localize(source: &str) -> Translation {
    convert(source, chinese)
}
fn convert(source: &str, lookup: fn(&str) -> &str) -> Translation {
    let lexed = tokenize(source);
    // 超过词法预算时保留全文，后续 inspect 会返回原始预算诊断。
    if lexed.tokens.is_empty() {
        return Translation {
            source: source.into(),
            segments: vec![Segment {
                original: Span::new(0, source.len()),
                generated: Span::new(0, source.len()),
            }],
            original_length: source.len(),
        };
    }
    let mut generated = String::new();
    let mut segments = Vec::with_capacity(lexed.tokens.len());
    for token in lexed.tokens {
        let start = generated.len();
        let text = token.text(source);
        generated.push_str(if token.kind == TokenKind::Identifier {
            lookup(text)
        } else {
            text
        });
        segments.push(Segment {
            original: token.span,
            generated: Span::new(start, generated.len()),
        });
    }
    Translation {
        source: generated,
        segments,
        original_length: source.len(),
    }
}
