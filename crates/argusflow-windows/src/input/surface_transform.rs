//! WGC frame-local物理坐标到虚拟屏幕物理坐标的边界转换。

use argusflow_agent::VisualTargetBounds;
use argusflow_core::{AutomationError, BackendKind, ScreenPoint};
use argusflow_vision::PhysicalRect;
use windows::Win32::Foundation::RECT;

/// 将 capture viewport 与 HWND 屏幕矩形绑定的纯物理坐标变换。
#[derive(Debug, Clone, Copy)]
pub(super) struct SurfaceTransform {
    /// WGC frame 中的局部物理 viewport。
    frame_viewport: PhysicalRect,
    /// frame viewport 左上角对应的 virtual-screen 物理坐标。
    viewport_origin: ScreenPoint,
    /// 当前捕获 viewport 在 virtual screen 中的安全输入范围。
    capture_screen_bounds: PhysicalRect,
}

/// 物化后的矩形和可直接注入的安全中心点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MappedSurfaceTarget {
    /// 虚拟屏幕物理坐标中的目标矩形。
    pub(super) bounds: VisualTargetBounds,
    /// 位于窗口内的安全点击点。
    pub(super) safe_point: ScreenPoint,
    /// 当前捕获 surface 在 virtual screen 中的输入范围。
    pub(super) surface_bounds: VisualTargetBounds,
}

impl SurfaceTransform {
    /// 创建使用已经归一化为物理像素的窗口和 frame 变换。
    #[cfg(test)]
    pub(super) fn new(
        window_bounds: RECT,
        frame_viewport: PhysicalRect,
    ) -> Result<Self, AutomationError> {
        let window_bounds = physical_rect(window_bounds)?;
        Self::new_with_origin(
            RECT {
                left: window_bounds.x,
                top: window_bounds.y,
                right: window_bounds.right() as i32,
                bottom: window_bounds.bottom() as i32,
            },
            frame_viewport,
            ScreenPoint {
                x: window_bounds.x,
                y: window_bounds.y,
            },
        )
    }

    /// 创建可处理 popup 合成帧的坐标变换；`viewport_origin` 是 frame viewport 的屏幕锚点。
    pub(super) fn new_with_origin(
        window_bounds: RECT,
        frame_viewport: PhysicalRect,
        viewport_origin: ScreenPoint,
    ) -> Result<Self, AutomationError> {
        let _ = physical_rect(window_bounds)?;
        let capture_screen_bounds = PhysicalRect::new(
            viewport_origin.x,
            viewport_origin.y,
            frame_viewport.width,
            frame_viewport.height,
        )
        .ok_or_else(|| invalid_mapping("capture viewport has an empty physical rectangle"))?;
        Ok(Self {
            frame_viewport,
            viewport_origin,
            capture_screen_bounds,
        })
    }

    /// 将 frame-local bbox 映射为虚拟屏幕物理矩形，并拒绝越界点击。
    pub(super) fn map_rect(
        self,
        frame_rect: PhysicalRect,
    ) -> Result<MappedSurfaceTarget, AutomationError> {
        if !frame_rect.is_inside(self.frame_viewport) {
            return Err(invalid_mapping(
                "visual bbox is outside the capture viewport",
            ));
        }
        let x = i64::from(self.viewport_origin.x) + i64::from(frame_rect.x)
            - i64::from(self.frame_viewport.x);
        let y = i64::from(self.viewport_origin.y) + i64::from(frame_rect.y)
            - i64::from(self.frame_viewport.y);
        let right = x + i64::from(frame_rect.width);
        let bottom = y + i64::from(frame_rect.height);
        let center_x = x + i64::from(frame_rect.width / 2);
        let center_y = y + i64::from(frame_rect.height / 2);
        if x < i64::from(i32::MIN)
            || y < i64::from(i32::MIN)
            || right > i64::from(i32::MAX)
            || bottom > i64::from(i32::MAX)
            || center_x < i64::from(self.capture_screen_bounds.x)
            || center_y < i64::from(self.capture_screen_bounds.y)
            || center_x >= self.capture_screen_bounds.right()
            || center_y >= self.capture_screen_bounds.bottom()
        {
            return Err(invalid_mapping(
                "visual target could not be mapped inside the target window",
            ));
        }
        Ok(MappedSurfaceTarget {
            bounds: VisualTargetBounds {
                x: x as i32,
                y: y as i32,
                width: frame_rect.width,
                height: frame_rect.height,
            },
            safe_point: ScreenPoint {
                x: center_x as i32,
                y: center_y as i32,
            },
            surface_bounds: VisualTargetBounds {
                x: self.capture_screen_bounds.x,
                y: self.capture_screen_bounds.y,
                width: self.capture_screen_bounds.width,
                height: self.capture_screen_bounds.height,
            },
        })
    }
}

/// 将 Win32 RECT 转为领域层物理矩形并校验非空范围。
fn physical_rect(bounds: RECT) -> Result<PhysicalRect, AutomationError> {
    let width = u32::try_from(i64::from(bounds.right) - i64::from(bounds.left))
        .map_err(|_| invalid_mapping("target window has an invalid horizontal extent"))?;
    let height = u32::try_from(i64::from(bounds.bottom) - i64::from(bounds.top))
        .map_err(|_| invalid_mapping("target window has an invalid vertical extent"))?;
    PhysicalRect::new(bounds.left, bounds.top, width, height)
        .ok_or_else(|| invalid_mapping("target window has an empty physical rectangle"))
}

/// 统一坐标契约错误，避免把映射边界问题误报为 OCR 或 SendInput 注入错误。
fn invalid_mapping(message: &str) -> AutomationError {
    AutomationError::BackendFailed {
        backend: BackendKind::SendInput,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_negative_virtual_screen_coordinates() {
        let transform = SurfaceTransform::new(
            RECT {
                left: -1920,
                top: -80,
                right: -120,
                bottom: 1000,
            },
            PhysicalRect::new(0, 0, 1800, 1080).unwrap(),
        )
        .unwrap();
        let mapped = transform
            .map_rect(PhysicalRect::new(300, 200, 100, 40).unwrap())
            .unwrap();

        assert_eq!(mapped.bounds.x, -1620);
        assert_eq!(mapped.bounds.y, 120);
        assert_eq!(mapped.safe_point, ScreenPoint { x: -1570, y: 140 });
    }

    #[test]
    fn treats_high_dpi_capture_dimensions_as_physical_pixels() {
        let transform = SurfaceTransform::new(
            RECT {
                left: 100,
                top: 200,
                right: 1900,
                bottom: 1400,
            },
            PhysicalRect::new(0, 0, 1800, 1200).unwrap(),
        )
        .unwrap();
        let mapped = transform
            .map_rect(PhysicalRect::new(600, 450, 200, 100).unwrap())
            .unwrap();

        assert_eq!(mapped.safe_point, ScreenPoint { x: 800, y: 700 });
    }

    #[test]
    fn rejects_a_bbox_outside_the_frame_viewport() {
        let transform = SurfaceTransform::new(
            RECT {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            },
            PhysicalRect::new(0, 0, 800, 600).unwrap(),
        )
        .unwrap();

        assert!(
            transform
                .map_rect(PhysicalRect::new(790, 590, 20, 20).unwrap())
                .is_err()
        );
    }
}
