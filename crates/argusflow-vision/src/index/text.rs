//! VisualScene 的 exact 文本倒排索引。

use std::collections::HashMap;

use crate::scene::VisualNode;

/// normalized text 到 scene node 索引的只读映射。
#[derive(Debug)]
pub struct TextIndex {
    /// exact normalized text 倒排表。
    by_exact: HashMap<String, Vec<usize>>,
}

impl TextIndex {
    /// 从 scene 节点构建倒排表。
    pub fn build(nodes: &[VisualNode]) -> Self {
        let mut by_exact: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, node) in nodes.iter().enumerate() {
            by_exact
                .entry(node.normalized_text.clone())
                .or_default()
                .push(index);
        }
        Self { by_exact }
    }

    /// 返回 exact normalized text 的节点索引。
    pub fn exact(&self, normalized_text: &str) -> &[usize] {
        self.by_exact
            .get(normalized_text)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}
