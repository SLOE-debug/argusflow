//! 滚动页面历史的显式去重与追加规则。

use serde::{Deserialize, Serialize};

use super::model::{PageItem, PageSnapshot};
use crate::scene::SceneId;

/// 一次页面追加的统计结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryAppend {
    /// 实际加入 history 的内容项数量。
    pub added: usize,
    /// 因 overlap 证明而跳过的内容项数量。
    pub deduplicated: usize,
}

/// 只保存已经接受页面的内容，不改变 current viewport scene。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScrollDocumentHistory {
    /// 按接受顺序排列的内容项。
    items: Vec<PageItem>,
    /// 已接受页面的 scene ID，供审计和分页指标使用。
    page_scene_ids: Vec<SceneId>,
}

impl ScrollDocumentHistory {
    /// 创建空历史。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回只读内容项视图。
    pub fn items(&self) -> &[PageItem] {
        &self.items
    }

    /// 返回已经接受的页面数量。
    pub fn page_count(&self) -> usize {
        self.page_scene_ids.len()
    }

    /// 追加一页；只有显式 overlap signature 才会触发去重。
    pub fn append_page(
        &mut self,
        page: &PageSnapshot,
        overlap_signatures: &std::collections::BTreeSet<u64>,
    ) -> HistoryAppend {
        let mut added = 0;
        let mut deduplicated = 0;
        for item in &page.items {
            if overlap_signatures.contains(&item.signature) {
                deduplicated += 1;
            } else {
                self.items.push(item.clone());
                added += 1;
            }
        }
        self.page_scene_ids.push(page.scene_id);
        HistoryAppend {
            added,
            deduplicated,
        }
    }
}
