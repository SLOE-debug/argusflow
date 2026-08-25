use serde::{Deserialize, Serialize};

/// WebView 文本协议中的位置；行与列均从零开始，列按 UTF-16 code unit 计数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EditorPosition {
    /// 从零开始的行号。
    pub line: u32,
    /// 从零开始的 UTF-16 code unit 列号。
    pub utf16_column: u32,
}

/// WebView 文本协议中的半开区间。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EditorRange {
    /// 区间起点。
    pub start: EditorPosition,
    /// 区间终点，不包含该位置。
    pub end: EditorPosition,
}

/// 将 Rust 内部 UTF-8 字节区间转换为浏览器安全的行号与 UTF-16 列号。
pub fn byte_range_to_editor_range(source: &str, start: usize, end: usize) -> EditorRange {
    EditorRange {
        start: byte_offset_to_position(source, start),
        end: byte_offset_to_position(source, end),
    }
}

/// 把一个合法或落在字符内部的字节偏移收敛到最近字符边界后进行转换。
fn byte_offset_to_position(source: &str, offset: usize) -> EditorPosition {
    let mut bounded_offset = offset.min(source.len());
    while !source.is_char_boundary(bounded_offset) {
        bounded_offset -= 1;
    }

    let prefix = &source[..bounded_offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let current_line = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, current_line)| current_line);
    let utf16_column = current_line.encode_utf16().count() as u32;

    EditorPosition { line, utf16_column }
}

#[cfg(test)]
mod tests {
    use super::{EditorPosition, EditorRange, byte_range_to_editor_range};

    #[test]
    fn converts_chinese_and_surrogate_pairs_to_utf16_positions() {
        let source = "button(name = \"保存😀\")\ntext()";
        let emoji_start = source.find('😀').expect("fixture contains emoji");
        let emoji_end = emoji_start + '😀'.len_utf8();

        assert_eq!(
            byte_range_to_editor_range(source, emoji_start, emoji_end),
            EditorRange {
                start: EditorPosition {
                    line: 0,
                    utf16_column: 17,
                },
                end: EditorPosition {
                    line: 0,
                    utf16_column: 19,
                },
            }
        );
    }
}
