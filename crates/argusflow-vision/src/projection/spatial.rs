//! SpatialText 的固定列数空间近似投影。

use crate::{projection::ProjectionOptions, scene::VisualScene};

/// 按 viewport 宽度把 node 映射到固定列数，保留 bbox 真值在 scene 中。
pub fn spatial_text(scene: &VisualScene, options: &ProjectionOptions) -> String {
    let columns = options.spatial_columns.max(1);
    let mut rows = scene
        .lines
        .iter()
        .map(|_| vec![None::<Cell>; columns])
        .collect::<Vec<_>>();
    for (row_index, line) in scene.lines.iter().enumerate() {
        for node_id in &line.node_ids {
            let Some(node) = scene
                .nodes
                .iter()
                .find(|candidate| candidate.id == *node_id)
            else {
                continue;
            };
            let col = (((node.bbox.x.max(scene.viewport.x) - scene.viewport.x) as f32
                / scene.viewport.width as f32)
                * columns as f32)
                .round()
                .clamp(0.0, (columns - 1) as f32) as usize;
            place_text(
                &mut rows[row_index],
                col,
                &node.normalized_text,
                node.confidence,
            );
        }
    }
    rows.into_iter()
        .map(|row| trim_row(row))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    /// 该列字符来自哪个 node 的置信度。
    confidence: f32,
    /// Unicode 字符。
    character: char,
}

/// 将文本逐字符放入空间列，冲突时保留高置信度 node。
fn place_text(row: &mut [Option<Cell>], start: usize, text: &str, confidence: f32) {
    let mut column = start;
    for character in text.chars() {
        let width = display_width(character);
        if column >= row.len() {
            break;
        }
        if row[column].is_none_or(|existing| confidence >= existing.confidence) {
            row[column] = Some(Cell {
                confidence,
                character,
            });
            if width == 2 && column + 1 < row.len() {
                row[column + 1] = Some(Cell {
                    confidence,
                    character: ' ',
                });
            }
        }
        column = column.saturating_add(width);
    }
}

/// 删除每行末尾 padding，但保留行间的换行关系。
fn trim_row(row: Vec<Option<Cell>>) -> String {
    let mut output = String::with_capacity(row.len());
    for cell in row {
        output.push(cell.map_or(' ', |value| value.character));
    }
    output.trim_end().to_owned()
}

/// 不依赖字体和 pt 的近似 Unicode display width。
fn display_width(character: char) -> usize {
    let code = character as u32;
    if matches!(
        code,
        0x1100..=0x115f
            | 0x2329..=0x232a
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
    ) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::display_width;

    #[test]
    fn cjk_characters_are_treated_as_double_width() {
        assert_eq!(display_width('中'), 2);
        assert_eq!(display_width('A'), 1);
    }
}
