//! 单个不可变 OCR Scene 的热路径查询索引。

use std::collections::HashMap;

use super::VisualNode;

/// 与 `VisualScene::nodes` 同生命周期构建的只读索引。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VisualSceneIndex {
    /// 规范化全文到阅读顺序节点下标的倒排表。
    exact_text: HashMap<String, Vec<usize>>,
}

impl VisualSceneIndex {
    /// 一次遍历构建全文索引；调用方必须先稳定节点阅读顺序。
    pub fn build(nodes: &[VisualNode]) -> Self {
        let mut exact_text = HashMap::<String, Vec<usize>>::new();
        for (index, node) in nodes.iter().enumerate() {
            exact_text
                .entry(node.normalized_text.clone())
                .or_default()
                .push(index);
        }
        Self { exact_text }
    }

    /// 返回规范化全文的节点下标，不分配临时集合。
    pub fn exact(&self, normalized_text: &str) -> &[usize] {
        self.exact_text
            .get(normalized_text)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}
