//! 基于垂直带和间距的联系人/消息 row 聚类。

use serde::{Deserialize, Serialize};

use crate::{
    frame::PhysicalRect,
    scene::{VisualNode, VisualNodeId},
};

/// 当前 scene 内的视觉 row ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VisualRowId(u64);

impl VisualRowId {
    /// 创建 row ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回 row ID 数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 一组属于同一联系人或消息行的视觉节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualRow {
    /// 短期 row ID。
    pub id: VisualRowId,
    /// 从左到右排列的行内节点。
    pub node_ids: Vec<VisualNodeId>,
    /// 覆盖整行的物理 bbox。
    pub bbox: PhysicalRect,
}

/// row 聚类的几何阈值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowConfig {
    /// 节点中心垂直差允许相对于高度的比例。
    pub center_tolerance_ratio: f32,
    /// 最小的物理像素容差。
    pub minimum_tolerance_px: f32,
}

impl Default for RowConfig {
    fn default() -> Self {
        Self {
            center_tolerance_ratio: 0.65,
            minimum_tolerance_px: 6.0,
        }
    }
}

/// 按中心 y 聚类 row，并把 row_id 写回节点。
pub fn cluster_rows(nodes: &mut [VisualNode], config: RowConfig) -> Vec<VisualRow> {
    let mut indexes = (0..nodes.len()).collect::<Vec<_>>();
    indexes.sort_by_key(|index| (nodes[*index].bbox.y, nodes[*index].bbox.x));
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    for index in indexes {
        let node = &nodes[index];
        let tolerance = (node.bbox.height as f32 * config.center_tolerance_ratio)
            .max(config.minimum_tolerance_px);
        let matching = buckets.iter().position(|bucket| {
            let reference = &nodes[bucket[0]];
            (reference.center().1 - node.center().1).abs()
                <= tolerance.max(reference.bbox.height as f32 * config.center_tolerance_ratio)
        });
        if let Some(bucket_index) = matching {
            buckets[bucket_index].push(index);
        } else {
            buckets.push(vec![index]);
        }
    }

    buckets
        .into_iter()
        .enumerate()
        .map(|(row_index, mut bucket)| {
            bucket.sort_by_key(|index| (nodes[*index].bbox.x, nodes[*index].bbox.y));
            let row_id = VisualRowId::new(row_index as u64 + 1);
            let bbox = bucket
                .iter()
                .skip(1)
                .fold(nodes[bucket[0]].bbox, |bounds, index| {
                    bounds.union(nodes[*index].bbox)
                });
            for index in &bucket {
                nodes[*index].row_id = Some(row_id);
            }
            VisualRow {
                id: row_id,
                node_ids: bucket.iter().map(|index| nodes[*index].id).collect(),
                bbox,
            }
        })
        .collect()
}
