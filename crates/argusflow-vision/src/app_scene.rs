//! 一个进程实例下多个独立窗口 Scene 的结构化集合。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{VisualNode, VisualScene, WindowDescriptor};

/// 一个顶层窗口及其最近一次稳定 OCR Scene。
#[derive(Debug, Clone)]
pub struct AppWindowScene {
    /// 枚举时冻结的窗口身份、边界和 Z-Order。
    pub window: WindowDescriptor,
    /// 该 HWND 独立捕获和构建的 Scene。
    pub scene: Arc<VisualScene>,
}

/// 一个进程当前全部可见顶层窗口的视觉事实。
#[derive(Debug, Clone)]
pub struct AppScene {
    /// 进程实例 PID。
    pub process_id: u32,
    /// 按桌面 Z-Order 排列的窗口 Scene。
    pub windows: Vec<AppWindowScene>,
}

impl AppScene {
    /// 以窗口和节点阅读顺序遍历全部 OCR 文本事实。
    pub fn nodes(&self) -> impl Iterator<Item = AppNodeRef<'_>> {
        self.windows.iter().flat_map(|window| {
            window.scene.nodes.iter().map(move |node| AppNodeRef {
                window: &window.window,
                scene: &window.scene,
                node,
            })
        })
    }
}

/// 跨窗口查询返回的节点引用，始终保留来源 HWND。
#[derive(Debug, Clone, Copy)]
pub struct AppNodeRef<'scene> {
    /// 节点所在窗口。
    pub window: &'scene WindowDescriptor,
    /// 节点所在不可变 Scene。
    pub scene: &'scene Arc<VisualScene>,
    /// OCR 文本节点。
    pub node: &'scene VisualNode,
}

/// 可序列化的进程窗口 Scene 摘要，用于 Inspector 和诊断，不包含像素。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSceneSummary {
    /// 进程 PID。
    pub process_id: u32,
    /// 已成功建立 Scene 的窗口数量。
    pub window_count: usize,
    /// 所有窗口 OCR 节点总数。
    pub node_count: usize,
}

impl From<&AppScene> for AppSceneSummary {
    fn from(scene: &AppScene) -> Self {
        Self {
            process_id: scene.process_id,
            window_count: scene.windows.len(),
            node_count: scene
                .windows
                .iter()
                .map(|window| window.scene.nodes.len())
                .sum(),
        }
    }
}

/// 已经通过 0/1/N 与置信度门槛的可持有文本目标。
#[derive(Debug, Clone)]
pub struct ResolvedTextTarget {
    /// 来源窗口。
    pub window: WindowDescriptor,
    /// 产生节点的不可变 Scene。
    pub scene: Arc<VisualScene>,
    /// 唯一命中的 OCR 节点。
    pub node: VisualNode,
}
