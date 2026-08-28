use std::sync::Arc;

use argusflow_agent::{
    ResolvedVisualTarget, VisualResolvePolicy, VisualTargetBounds, VisualTargetResolver,
    WindowContext,
};
use argusflow_core::{AutomationError, ScreenPoint, VisualQuery, WindowIdentity};
use argusflow_vision::{
    SceneRefreshPolicy, VisionError, VisionRuntime, VisualMatch, evaluate_visual_query,
};
use async_trait::async_trait;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::GetWindowRect,
};

/// 基于共享 VisionRuntime 的 Windows 视觉目标解析器。
#[derive(Debug, Clone)]
pub struct WindowsVisualTargetResolver {
    /// 与视觉读取后端共享 capture、OCR worker 和 scene cache。
    runtime: Arc<VisionRuntime>,
}

impl WindowsVisualTargetResolver {
    /// 创建绑定共享视觉运行时的解析器。
    pub fn new(runtime: Arc<VisionRuntime>) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl VisualTargetResolver for WindowsVisualTargetResolver {
    async fn resolve(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
        policy: VisualResolvePolicy,
    ) -> Result<ResolvedVisualTarget, AutomationError> {
        let mut refresh = if policy.prefer_medium {
            SceneRefreshPolicy::medium()
        } else {
            SceneRefreshPolicy::tiny()
        };
        refresh.force_refresh = policy.force_refresh;
        refresh.normalized_query_region = query.region;
        let identity = WindowIdentity {
            handle: window.handle,
            process_id: window.process_id,
        };
        let scene = self
            .runtime
            .current_scene(identity, &refresh)
            .await
            .map_err(map_vision_error)?;
        if scene.window != identity {
            return Err(AutomationError::BackendFailed {
                backend: argusflow_core::BackendKind::SendInput,
                message: "visual scene belongs to a different window identity".to_owned(),
            });
        }
        let VisualMatch::Unique(node) = evaluate_visual_query(&scene, query)?;
        let bounds = window_bounds(window)?;
        materialize_target(window, &scene, node, bounds)
    }
}

/// 从当前窗口矩形和 frame-local bbox 计算虚拟屏幕物理目标。
fn materialize_target(
    window: &WindowContext,
    scene: &argusflow_vision::VisualScene,
    node: &argusflow_vision::VisualNode,
    window_bounds: RECT,
) -> Result<ResolvedVisualTarget, AutomationError> {
    let x = i64::from(window_bounds.left) - i64::from(scene.viewport.x) + i64::from(node.bbox.x);
    let y = i64::from(window_bounds.top) - i64::from(scene.viewport.y) + i64::from(node.bbox.y);
    let right = x + i64::from(node.bbox.width);
    let bottom = y + i64::from(node.bbox.height);
    let center_x = x + i64::from(node.bbox.width / 2);
    let center_y = y + i64::from(node.bbox.height / 2);
    if x < i64::from(i32::MIN)
        || y < i64::from(i32::MIN)
        || right > i64::from(i32::MAX)
        || bottom > i64::from(i32::MAX)
        || center_x < i64::from(window_bounds.left)
        || center_y < i64::from(window_bounds.top)
        || center_x >= i64::from(window_bounds.right)
        || center_y >= i64::from(window_bounds.bottom)
    {
        return Err(AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::SendInput,
            message: "visual target could not be mapped inside the target window".to_owned(),
        });
    }
    Ok(ResolvedVisualTarget {
        window: window.clone(),
        scene_id: scene.scene_id.get(),
        frame_id: scene.frame_id.get(),
        bounds: VisualTargetBounds {
            x: x as i32,
            y: y as i32,
            width: node.bbox.width,
            height: node.bbox.height,
        },
        confidence: node.confidence,
        safe_point: ScreenPoint {
            x: center_x as i32,
            y: center_y as i32,
        },
    })
}

/// 读取已验证 HWND 的屏幕矩形。
fn window_bounds(window: &WindowContext) -> Result<RECT, AutomationError> {
    let hwnd = HWND(window.handle as usize as *mut std::ffi::c_void);
    let mut bounds = RECT::default();
    // SAFETY: bounds 是同步 Win32 调用的独占输出，HWND 由 AppSession/前台上下文提供。
    unsafe { GetWindowRect(hwnd, &mut bounds) }
        .map(|_| bounds)
        .map_err(|error| AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::SendInput,
            message: format!("failed to read target window bounds: {error}"),
        })
}

/// 把视觉管线错误转换成可由 Planner 识别的 SendInput 前置失败。
fn map_vision_error(error: VisionError) -> AutomationError {
    match error {
        VisionError::CaptureUnavailable { message }
        | VisionError::WorkerUnavailable { message }
        | VisionError::OcrFailed { message }
        | VisionError::Protocol { message } => AutomationError::BackendUnavailable {
            backend: argusflow_core::BackendKind::SendInput,
            message,
        },
        other => AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::SendInput,
            message: other.to_string(),
        },
    }
}
