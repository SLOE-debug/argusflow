//! 基于 bbox 的确定性视觉行聚类。

use serde::{Deserialize, Serialize};

use crate::{
    frame::PhysicalRect,
    scene::{VisualNode, VisualNodeId},
};

/// 当前 scene 内的视觉行 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VisualLineId(u64);

impl VisualLineId {
    /// 创建行 ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回行 ID 数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 一组垂直位置相近、水平相邻的 OCR node。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualLine {
    /// 短期行 ID。
    pub id: VisualLineId,
    /// 行内从左到右排列的节点。
    pub node_ids: Vec<VisualNodeId>,
    /// 包含所有行内节点的 bbox。
    pub bbox: PhysicalRect,
    /// 使用确定性 gap 规则拼出的行文本。
    pub text: String,
}

/// 将 node 按垂直中心聚成视觉行，并把 line_id 写回节点。
pub fn cluster_lines(nodes: &mut [VisualNode]) -> Vec<VisualLine> {
    let mut indexes = (0..nodes.len()).collect::<Vec<_>>();
    indexes.sort_by(|left, right| {
        nodes[*left]
            .bbox
            .y
            .cmp(&nodes[*right].bbox.y)
            .then_with(|| nodes[*left].bbox.x.cmp(&nodes[*right].bbox.x))
    });
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    for index in indexes {
        let node = &nodes[index];
        let center_y = node.center().1;
        let tolerance = (node.bbox.height as f32 * 0.6).max(4.0);
        let matching_bucket = buckets.iter().position(|bucket| {
            let reference = &nodes[bucket[0]];
            (reference.center().1 - center_y).abs()
                <= tolerance.max(reference.bbox.height as f32 * 0.6)
        });
        if let Some(bucket_index) = matching_bucket {
            buckets[bucket_index].push(index);
        } else {
            buckets.push(vec![index]);
        }
    }

    let mut lines = buckets
        .into_iter()
        .enumerate()
        .map(|(line_index, mut bucket)| {
            bucket.sort_by_key(|index| (nodes[*index].bbox.x, nodes[*index].bbox.y));
            let line_id = VisualLineId::new(line_index as u64 + 1);
            let first_rect = nodes[bucket[0]].bbox;
            let line_bbox = bucket
                .iter()
                .skip(1)
                .fold(first_rect, |bounds, index| bounds.union(nodes[*index].bbox));
            for index in &bucket {
                nodes[*index].line_id = Some(line_id);
            }
            let text = join_line_nodes(&bucket, nodes);
            VisualLine {
                id: line_id,
                node_ids: bucket.iter().map(|index| nodes[*index].id).collect(),
                bbox: line_bbox,
                text,
            }
        })
        .collect::<Vec<_>>();
    lines.sort_by_key(|line| (line.bbox.y, line.bbox.x, line.id.get()));
    lines
}

/// 按 bbox gap 选择普通空格或跨列制表符。
fn join_line_nodes(indexes: &[usize], nodes: &[VisualNode]) -> String {
    let mut text = String::new();
    for (position, index) in indexes.iter().copied().enumerate() {
        if position > 0 {
            let previous = &nodes[indexes[position - 1]];
            let current = &nodes[index];
            let gap = current.bbox.x as i64 - previous.bbox.right();
            let separator = if gap > i64::from(previous.bbox.height.max(1)) * 2 {
                '\t'
            } else {
                ' '
            };
            text.push(separator);
        }
        text.push_str(&nodes[index].normalized_text);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ocr::{OcrSource, PolygonPoint},
        scene::VisualNode,
    };

    fn node(text: &str, x: f32, y: f32) -> VisualNode {
        VisualNode::from_ocr(
            crate::scene::SceneId::new(1),
            text.to_owned(),
            0.9,
            vec![
                PolygonPoint { x, y },
                PolygonPoint { x: x + 20.0, y },
                PolygonPoint {
                    x: x + 20.0,
                    y: y + 10.0,
                },
                PolygonPoint { x, y: y + 10.0 },
            ],
            OcrSource::OcrTiny,
            None,
        )
        .expect("fixture node is valid")
    }

    #[test]
    fn line_clustering_preserves_order_and_gap_semantics() {
        let mut nodes = vec![
            node("联系人", 0.0, 0.0),
            node("10:21", 80.0, 0.0),
            node("正文", 0.0, 30.0),
        ];
        let lines = cluster_lines(&mut nodes);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "联系人\t10:21");
        assert_eq!(lines[1].text, "正文");
        assert!(nodes.iter().all(|node| node.line_id.is_some()));
    }
}
