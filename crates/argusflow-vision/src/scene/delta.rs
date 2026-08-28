//! 相邻 VisualScene 的确定性语义差分。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::VisionError;

use super::{SceneId, VisualNode, VisualNodeId, VisualScene};

/// 一个视觉节点在相邻 scene 中保持同一 stable identity 但事实属性发生变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualNodeChange {
    /// 前一份 scene 中的节点。
    pub before: VisualNode,
    /// 当前 scene 中的节点。
    pub after: VisualNode,
}

/// 相邻 scene 的语义差分；不把滚动历史混入 current viewport。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualSceneDelta {
    /// 前一份 scene 的 ID。
    pub from_scene_id: SceneId,
    /// 当前 scene 的 ID。
    pub to_scene_id: SceneId,
    /// 当前 scene 新增的节点。
    pub added: Vec<VisualNode>,
    /// 当前 scene 已经消失的节点 ID。
    pub removed: Vec<VisualNodeId>,
    /// stable identity 未变但事实属性有变化的节点。
    pub changed: Vec<VisualNodeChange>,
}

impl VisualSceneDelta {
    /// 判断当前 scene 是否存在新节点。
    pub fn has_additions(&self) -> bool {
        !self.added.is_empty()
    }

    /// 判断两个 scene 是否在节点事实层面完全相同。
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// 在同一窗口和兼容拓扑代数内计算 deterministic scene delta。
pub fn diff_scenes(
    previous: &VisualScene,
    current: &VisualScene,
) -> Result<VisualSceneDelta, VisionError> {
    if previous.window != current.window {
        return Err(VisionError::WindowIdentityChanged {
            expected: previous.window,
            actual: Some(current.window),
        });
    }
    if !previous.topology_generation.is_unknown()
        && !current.topology_generation.is_unknown()
        && previous.topology_generation != current.topology_generation
    {
        return Err(VisionError::OcrCancelled {
            reason: "cannot diff scenes from different window topology generations".to_owned(),
        });
    }

    let previous_nodes = index_nodes(&previous.nodes);
    let current_nodes = index_nodes(&current.nodes);
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for (node_id, node) in &current_nodes {
        match previous_nodes.get(node_id) {
            None => added.push((*node).clone()),
            Some(previous_node) if !same_node_facts(previous_node, node) => {
                changed.push(VisualNodeChange {
                    before: (*previous_node).clone(),
                    after: (*node).clone(),
                })
            }
            Some(_) => {}
        }
    }
    for node_id in previous_nodes.keys() {
        if !current_nodes.contains_key(node_id) {
            removed.push(*node_id);
        }
    }

    added.sort_by_key(|node| (node.bbox.y, node.bbox.x, node.id));
    removed.sort();
    changed.sort_by_key(|change| (change.after.bbox.y, change.after.bbox.x, change.after.id));
    Ok(VisualSceneDelta {
        from_scene_id: previous.scene_id,
        to_scene_id: current.scene_id,
        added,
        removed,
        changed,
    })
}

/// 以 node ID 建立只读索引，重复 ID 保留排序后第一项以保证确定性。
fn index_nodes(nodes: &[VisualNode]) -> BTreeMap<VisualNodeId, &VisualNode> {
    nodes.iter().fold(BTreeMap::new(), |mut index, node| {
        index.entry(node.id).or_insert(node);
        index
    })
}

/// 判断节点的观测事实是否变化；scene generation 是快照元数据，不应制造假 delta。
fn same_node_facts(left: &VisualNode, right: &VisualNode) -> bool {
    left.id == right.id
        && left.raw_text == right.raw_text
        && left.normalized_text == right.normalized_text
        && left.role_hint == right.role_hint
        && left.bbox == right.bbox
        && left.polygon == right.polygon
        && left.confidence == right.confidence
        && left.source == right.source
        && left.region_id == right.region_id
        && left.line_id == right.line_id
        && left.row_id == right.row_id
        && left.stable_hash == right.stable_hash
}
