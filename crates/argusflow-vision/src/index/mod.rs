//! VisualScene 的只读文本、几何与 row 索引。

mod geometry;
mod text;

use std::{collections::HashMap, sync::Arc};

use crate::{
    layout::VisualRowId,
    scene::{VisualNode, VisualNodeId, VisualScene},
};

pub use geometry::{center_distance_normalized, direction_matches, edge_gap_normalized};
pub use text::TextIndex;

/// AQL Vision executor 查询的结构化场景索引。
#[derive(Debug)]
pub struct VisualSceneIndex {
    /// 索引绑定的不可变 scene。
    scene: Arc<VisualScene>,
    /// exact/contains 文本候选索引。
    text: TextIndex,
    /// 按 reading order 保存的全部节点索引。
    geometry_order: Vec<usize>,
    /// row 到节点索引的映射。
    by_row: HashMap<VisualRowId, Vec<usize>>,
}

impl VisualSceneIndex {
    /// 从完整结构化 scene 构建轻量内存索引。
    pub fn build(scene: Arc<VisualScene>) -> Self {
        let text = TextIndex::build(&scene.nodes);
        let mut geometry_order = (0..scene.nodes.len()).collect::<Vec<_>>();
        geometry_order.sort_by_key(|index| {
            let node = &scene.nodes[*index];
            (node.bbox.y, node.bbox.x, node.id)
        });
        let mut by_row: HashMap<VisualRowId, Vec<usize>> = HashMap::new();
        for (index, node) in scene.nodes.iter().enumerate() {
            if let Some(row_id) = node.row_id {
                by_row.entry(row_id).or_default().push(index);
            }
        }
        Self {
            scene,
            text,
            geometry_order,
            by_row,
        }
    }

    /// 返回索引绑定的结构化事实。
    pub fn scene(&self) -> &VisualScene {
        &self.scene
    }

    /// 执行 exact 文本倒排查询。
    pub fn exact_text(&self, normalized_text: &str) -> Vec<&VisualNode> {
        self.text
            .exact(normalized_text)
            .iter()
            .map(|index| &self.scene.nodes[*index])
            .collect()
    }

    /// 执行 contains residual 查询，候选保持 reading order。
    pub fn contains_text(&self, normalized_text: &str) -> Vec<&VisualNode> {
        self.geometry_order
            .iter()
            .map(|index| &self.scene.nodes[*index])
            .filter(|node| node.normalized_text.contains(normalized_text))
            .collect()
    }

    /// 返回同一 row 的结构化节点；缺少 row 时返回空集合。
    pub fn row_nodes(&self, row_id: VisualRowId) -> Vec<&VisualNode> {
        self.by_row
            .get(&row_id)
            .into_iter()
            .flatten()
            .map(|index| &self.scene.nodes[*index])
            .collect()
    }

    /// 按 node ID 获取当前 scene 内节点。
    pub fn node(&self, id: VisualNodeId) -> Option<&VisualNode> {
        self.scene.nodes.iter().find(|node| node.id == id)
    }
}

/// 同时冻结 scene、索引和观测完整性的一致性快照。
#[derive(Debug)]
pub struct VisualSceneSnapshot {
    /// 不可变场景事实。
    pub scene: Arc<VisualScene>,
    /// 由同一 scene 构建的查询索引。
    pub index: Arc<VisualSceneIndex>,
    /// 构建 snapshot 时的观测状态。
    pub observation: crate::scene::ObservationState,
}

impl VisualSceneSnapshot {
    /// 从同一 scene 与 cache observation 创建一致性快照。
    pub fn new(scene: Arc<VisualScene>, observation: crate::scene::ObservationState) -> Self {
        let index = Arc::new(VisualSceneIndex::build(scene.clone()));
        Self {
            scene,
            index,
            observation,
        }
    }
}
