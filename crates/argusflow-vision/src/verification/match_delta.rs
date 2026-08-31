//! 动作前后 AQL 匹配实例的空间差分。

use crate::VisualNode;

/// 统计当前匹配中不与任一同文本历史实例相交的节点。
pub(super) fn added_match_count(baseline: &[VisualNode], current: &[VisualNode]) -> usize {
    current
        .iter()
        .filter(|candidate| {
            !baseline.iter().any(|historical| {
                historical.normalized_text == candidate.normalized_text
                    && historical.bbox.intersects(candidate.bbox)
            })
        })
        .count()
}

#[cfg(test)]
mod tests {
    use crate::{PolygonPoint, SceneId, VisualNode, VisualNodeSource};

    use super::added_match_count;

    /// 创建用于空间差分的最小 OCR 节点。
    fn node(text: &str, x: f32, y: f32) -> VisualNode {
        VisualNode::from_ocr(
            SceneId::new(1),
            text.to_owned(),
            0.99,
            vec![
                PolygonPoint { x, y },
                PolygonPoint { x: x + 20.0, y },
                PolygonPoint {
                    x: x + 20.0,
                    y: y + 10.0,
                },
                PolygonPoint { x, y: y + 10.0 },
            ],
            VisualNodeSource::OcrSmall,
        )
        .expect("test node is valid")
    }

    #[test]
    fn overlapping_same_text_is_an_existing_instance() {
        let baseline = vec![node("已发送", 10.0, 10.0)];
        let current = vec![node("已发送", 12.0, 11.0)];

        assert_eq!(added_match_count(&baseline, &current), 0);
    }

    #[test]
    fn non_overlapping_same_text_is_a_new_instance() {
        let baseline = vec![node("重复消息", 10.0, 10.0)];
        let current = vec![node("重复消息", 12.0, 11.0), node("重复消息", 10.0, 60.0)];

        assert_eq!(added_match_count(&baseline, &current), 1);
    }

    #[test]
    fn changed_text_never_reuses_an_overlapping_instance() {
        let baseline = vec![node("旧消息", 10.0, 10.0)];
        let current = vec![node("新消息", 10.0, 10.0)];

        assert_eq!(added_match_count(&baseline, &current), 1);
    }
}
