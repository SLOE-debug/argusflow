//! 结构化 Scene 到运行执行台的惰性只读投影。

use argusflow_core::ScreenPoint;
use serde::Serialize;

use crate::{AppScene, PhysicalRect, PolygonPoint, VisualNodeSource};

/// 一次查询所见完整进程 Scene 的真实坐标投影。
#[derive(Debug, Clone, Serialize)]
pub struct SceneProjection {
    /// 投影协议版本；v2 不再包含字符网格近似。
    pub schema_version: u16,
    /// 保留桌面边界与层级的全部窗口。
    pub windows: Vec<SceneWindowProjection>,
    /// 可在文字地图、截图与坐标表中共用的 OCR 节点。
    pub nodes: Vec<SceneNodeProjection>,
}

/// 单个窗口的桌面位置、层级和捕获帧身份。
#[derive(Debug, Clone, Serialize)]
pub struct SceneWindowProjection {
    pub window_handle: u64,
    pub scene_id: u64,
    pub frame_id: u64,
    pub z_order: usize,
    pub foreground: bool,
    pub screen_bounds: PhysicalRect,
    pub frame_bounds: PhysicalRect,
}

/// 单个 OCR 文本节点的帧坐标、屏幕坐标与质量事实。
#[derive(Debug, Clone, Serialize)]
pub struct SceneNodeProjection {
    pub node_id: String,
    pub scene_id: u64,
    pub frame_id: u64,
    pub window_handle: u64,
    pub text: String,
    pub frame_bbox: PhysicalRect,
    pub screen_bbox: PhysicalRect,
    pub polygon: Vec<PolygonPoint>,
    pub confidence: f32,
    pub source: VisualNodeSource,
}

/// 结构化候选身份，避免不同窗口内重复的节点 ID 相互碰撞。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneNodeIdentity {
    pub window_handle: u64,
    pub scene_id: u64,
    pub node_id: String,
}

/// 只有 trace sink 请求时才生成真实坐标集合。
pub fn project_app_scene(scene: &AppScene) -> SceneProjection {
    let mut windows = Vec::with_capacity(scene.windows.len());
    let mut nodes = Vec::new();
    for window in &scene.windows {
        windows.push(SceneWindowProjection {
            window_handle: window.window.identity.handle,
            scene_id: window.scene.scene_id.get(),
            frame_id: window.scene.frame_id.get(),
            z_order: window.window.z_order,
            foreground: window.window.foreground,
            screen_bounds: window.window.screen_bounds,
            frame_bounds: window.scene.viewport,
        });
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
        schema_version: 2,
        windows,
        nodes,
    }
}

fn translate_rect(rect: PhysicalRect, origin: ScreenPoint) -> PhysicalRect {
    PhysicalRect {
        x: rect.x.saturating_add(origin.x),
        y: rect.y.saturating_add(origin.y),
        width: rect.width,
        height: rect.height,
    }
}

#[cfg(test)]
mod tests {
    use argusflow_core::WindowIdentity;

    use crate::{
        AppScene, AppWindowScene, CapturedFrame, FrameId, OcrItem, OcrModel,
        OcrPreprocessingSummary, OcrRequestId, OcrResponse, OcrTimingSummary, PhysicalRect,
        PolygonPoint, QpcTimestamp, SceneBuildOptions, TopologyGeneration, VisualSceneBuilder,
        WindowDescriptor,
    };

    use super::project_app_scene;

    #[test]
    fn v2_projection_preserves_window_bounds_and_real_text_coordinates() {
        let scene = AppScene {
            process_id: 7,
            windows: vec![window_scene(101, 40, 25, "搜索联系人")],
        };

        let projection = project_app_scene(&scene);

        assert_eq!(projection.schema_version, 2);
        assert_eq!(projection.windows[0].screen_bounds.x, 40);
        assert_eq!(projection.windows[0].screen_bounds.y, 25);
        assert_eq!(projection.nodes[0].screen_bbox.x, 52);
        assert_eq!(projection.nodes[0].screen_bbox.y, 43);
        assert_eq!(projection.nodes[0].text, "搜索联系人");
    }

    fn window_scene(handle: u64, screen_x: i32, screen_y: i32, text: &str) -> AppWindowScene {
        let identity = WindowIdentity {
            handle,
            process_id: 7,
        };
        let frame = CapturedFrame::from_bgra8(
            FrameId::new(1),
            TopologyGeneration::new(1),
            identity,
            QpcTimestamp::new(1),
            200,
            120,
            96,
            96,
            800,
            vec![0; 200 * 120 * 4],
        )
        .expect("fixture frame should be valid")
        .with_screen_origin(argusflow_core::ScreenPoint {
            x: screen_x,
            y: screen_y,
        });
        let response = OcrResponse {
            request_id: OcrRequestId::new(),
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            model: OcrModel::PpOcrV6Small,
            elapsed_ms: 1,
            preprocessing: OcrPreprocessingSummary {
                input_width: 200,
                input_height: 120,
                output_width: 200,
                output_height: 120,
                contrast_enhanced: false,
                sharpened: false,
                binarized: false,
            },
            timings: OcrTimingSummary {
                preprocess_elapsed_ms: 0,
                inference_elapsed_ms: 1,
            },
            model_input: None,
            items: vec![OcrItem {
                raw_text: text.to_owned(),
                confidence: 0.98,
                polygon: vec![
                    PolygonPoint { x: 12.0, y: 18.0 },
                    PolygonPoint { x: 80.0, y: 18.0 },
                    PolygonPoint { x: 80.0, y: 36.0 },
                    PolygonPoint { x: 12.0, y: 36.0 },
                ],
            }],
        };
        let scene = VisualSceneBuilder::new()
            .build(identity, &frame, &[response], &SceneBuildOptions::default())
            .expect("fixture scene should build");
        AppWindowScene {
            window: WindowDescriptor {
                identity,
                owner_handle: None,
                z_order: 0,
                screen_bounds: PhysicalRect::new(screen_x, screen_y, 200, 120)
                    .expect("valid bounds"),
                foreground: true,
            },
            scene,
        }
    }
}
