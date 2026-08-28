use argusflow_core::{ScreenPoint, VisualQuery};
use async_trait::async_trait;

use crate::WindowContext;

/// 视觉目标解析时使用的刷新偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualResolvePolicy {
    /// 是否跳过可复用场景并重新取得稳定画面。
    pub force_refresh: bool,
    /// 是否优先使用高精度视觉模型。
    pub prefer_medium: bool,
}

impl Default for VisualResolvePolicy {
    fn default() -> Self {
        Self {
            force_refresh: true,
            prefer_medium: true,
        }
    }
}

/// 已由视觉场景物化、可交给 SendInput 的屏幕物理矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualTargetBounds {
    /// 左边界，单位为虚拟屏幕物理像素。
    pub x: i32,
    /// 上边界，单位为虚拟屏幕物理像素。
    pub y: i32,
    /// 宽度，单位为物理像素。
    pub width: u32,
    /// 高度，单位为物理像素。
    pub height: u32,
}

/// 一次视觉解析得到的目标事实。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedVisualTarget {
    /// 解析期间绑定的窗口身份。
    pub window: WindowContext,
    /// 产生该事实的 scene generation。
    pub scene_id: u64,
    /// 产生该事实的 capture frame。
    pub frame_id: u64,
    /// 目标在虚拟屏幕物理坐标中的 bbox。
    pub bounds: VisualTargetBounds,
    /// OCR 或视觉节点提供的置信度。
    pub confidence: f32,
    /// 经边界校验、适合实际点击的安全点。
    pub safe_point: ScreenPoint,
}

/// Visual Click 通过该窄接口把观察和物理输入解耦。
#[async_trait]
pub trait VisualTargetResolver: Send + Sync {
    /// 在指定窗口的最新稳定场景中严格解析一个视觉目标。
    async fn resolve(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
        policy: VisualResolvePolicy,
    ) -> Result<ResolvedVisualTarget, argusflow_core::AutomationError>;
}
