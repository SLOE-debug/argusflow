//! VisualScene region 领域模型。

use serde::{Deserialize, Serialize};

use crate::frame::PhysicalRect;

use super::node::VisualNodeId;

/// scene 内 region 的稳定短期 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VisualRegionId(u64);

impl VisualRegionId {
    /// 创建 region ID。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回 region ID 数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 通用视觉布局区域；微信 profile 只提供弱先验，不写死坐标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualRegionKind {
    /// 导航区域。
    Navigation,
    /// 侧栏区域。
    Sidebar,
    /// 列表区域。
    List,
    /// 顶部标题区域。
    Header,
    /// 一般内容区域。
    Content,
    /// 聊天历史区域。
    ChatHistory,
    /// 输入编辑器区域。
    Editor,
    /// 独立或叠加弹出层。
    Popup,
    /// 对话框。
    Dialog,
    /// 尚未分类的区域。
    Unknown,
}

/// 带有弱语义分类的视觉区域。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualRegion {
    /// 短期 region ID。
    pub id: VisualRegionId,
    /// 区域类型。
    pub kind: VisualRegionKind,
    /// 帧本地物理像素范围。
    pub bounds: PhysicalRect,
    /// 属于该区域的 node IDs。
    pub node_ids: Vec<VisualNodeId>,
}

impl VisualRegion {
    /// 创建区域。
    pub fn new(id: VisualRegionId, kind: VisualRegionKind, bounds: PhysicalRect) -> Self {
        Self {
            id,
            kind,
            bounds,
            node_ids: Vec::new(),
        }
    }
}
