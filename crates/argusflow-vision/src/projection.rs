//! 结构化 Scene 到 Run Inspector 的惰性只读投影。

use argusflow_core::ScreenPoint;
use serde::Serialize;

use crate::{AppScene, PhysicalRect, PolygonPoint, VisualNodeSource};

/// 一次查询所见完整进程 Scene 的可序列化投影。
#[derive(Debug, Clone, Serialize)]
pub struct SceneProjection {
    /// 投影协议版本。
    pub schema_version: u16,
    /// 近似 2D 空间文字，仅用于观察与人工定位。
    pub spatial_text: String,
    /// 精确节点坐标表；执行点击只使用这里对应的 Scene 事实。
    pub nodes: Vec<SceneNodeProjection>,
}

/// 单个 OCR 文本节点的帧坐标、屏幕坐标与质量事实。
#[derive(Debug, Clone, Serialize)]
pub struct SceneNodeProjection {
    /// Scene 节点稳定 ID。
    pub node_id: String,
    /// 所属 Scene ID。
    pub scene_id: u64,
    /// 所属捕获帧 ID。
    pub frame_id: u64,
    /// 所属顶层窗口句柄。
    pub window_handle: u64,
    /// OCR 原始文字。
    pub text: String,
    /// 捕获帧本地物理像素边界。
    pub frame_bbox: PhysicalRect,
    /// 虚拟屏幕物理像素边界。
    pub screen_bbox: PhysicalRect,
    /// OCR 原始 Polygon。
    pub polygon: Vec<PolygonPoint>,
    /// OCR 置信度。
    pub confidence: f32,
    /// 实际提供该节点的 OCR 模型。
    pub source: VisualNodeSource,
}

/// 只有 trace sink 请求时才生成近似布局和精确坐标集合。
pub fn project_app_scene(scene: &AppScene) -> SceneProjection {
    let mut spatial_sections = Vec::with_capacity(scene.windows.len());
    let mut nodes = Vec::new();
    for window in &scene.windows {
        spatial_sections.push(project_window_layout(
            window.window.identity.handle,
            window.scene.viewport,
            &window.scene.nodes,
        ));
        for node in &window.scene.nodes {
            nodes.push(SceneNodeProjection {
                node_id: node.id.get().to_string(),
                scene_id: window.scene.scene_id.get(),
                frame_id: window.scene.frame_id.get(),
                window_handle: window.window.identity.handle,
                text: node.raw_text.clone(),
                frame_bbox: node.bbox,
                screen_bbox: translate_rect(node.bbox, window.scene.viewport_origin),
                polygon: node.polygon.clone(),
                confidence: node.confidence,
                source: node.source,
            });
        }
    }
    SceneProjection {
        schema_version: 1,
        spatial_text: spatial_sections.join("\n\n"),
        nodes,
    }
}

fn project_window_layout(
    handle: u64,
    viewport: PhysicalRect,
    nodes: &[crate::VisualNode],
) -> String {
    const COLUMNS: usize = 120;
    let mut lines = vec![format!(
        "window {handle} · {}×{}",
        viewport.width, viewport.height
    )];
    let mut current_bottom = i64::MIN;
    let mut row = vec![' '; COLUMNS];
    for node in nodes {
        if i64::from(node.bbox.y) >= current_bottom && row.iter().any(|character| *character != ' ')
        {
            lines.push(row.iter().collect::<String>().trim_end().to_owned());
            row.fill(' ');
        }
        current_bottom = current_bottom.max(node.bbox.bottom());
        let relative_x = i64::from(node.bbox.x) - i64::from(viewport.x);
        let column = ((relative_x.max(0) as u128 * COLUMNS as u128)
            / u128::from(viewport.width.max(1))) as usize;
        for (offset, character) in node.raw_text.chars().enumerate() {
            let index = column.saturating_add(offset);
            if index >= COLUMNS {
                break;
            }
            row[index] = character;
        }
    }
    if row.iter().any(|character| *character != ' ') {
        lines.push(row.iter().collect::<String>().trim_end().to_owned());
    }
    lines.join("\n")
}

fn translate_rect(rect: PhysicalRect, origin: ScreenPoint) -> PhysicalRect {
    PhysicalRect {
        x: rect.x.saturating_add(origin.x),
        y: rect.y.saturating_add(origin.y),
        width: rect.width,
        height: rect.height,
    }
}
