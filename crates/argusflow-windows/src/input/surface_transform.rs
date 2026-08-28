//! WGC frame-local物理坐标到虚拟屏幕物理坐标的边界转换。

use argusflow_agent::VisualTargetBounds;
use argusflow_core::{AutomationError, BackendKind, ScreenPoint};
use argusflow_vision::PhysicalRect;
use windows::Win32::Foundation::RECT;

/// 将 capture viewport 与 HWND 屏幕矩形绑定的纯物理坐标变换。
#[derive(Debug, Clone, Copy)]
pub(super) struct SurfaceTransform {
    /// 当前窗口在 virtual screen 中的物理矩形。
    window_bounds: PhysicalRect,
    /// WGC frame 中的局部物理 viewport。
    frame_viewport: PhysicalRect,
}

/// 物化后的矩形和可直接注入的安全中心点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MappedSurfaceTarget {
    /// 虚拟屏幕物理坐标中的目标矩形。
    pub(super) bounds: VisualTargetBounds,
    /// 位于窗口内的安全点击点。
    pub(super) safe_point: ScreenPoint,
}

impl SurfaceTransform {
    /// 创建使用已经归一化为物理像素的窗口和 frame 变换。
    pub(super) fn new(
        window_bounds: RECT,
        frame_viewport: PhysicalRect,
    ) -> Result<Self, AutomationError> {
        let width =
            u32::try_from(i64::from(window_bounds.right) - i64::from(window_bounds.left))
                .map_err(|_| invalid_mapping("target window has an invalid horizontal extent"))?;
        let height = u32::try_from(i64::from(window_bounds.bottom) - i64::from(window_bounds.top))
            .map_err(|_| invalid_mapping("target window has an invalid vertical extent"))?;
        let window_bounds = PhysicalRect::new(window_bounds.left, window_bounds.top, width, height)
            .ok_or_else(|| invalid_mapping("target window has an empty physical rectangle"))?;
        Ok(Self {
            window_bounds,
            frame_viewport,
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
        let x = i64::from(self.window_bounds.x) + i64::from(frame_rect.x)
            - i64::from(self.frame_viewport.x);
        let y = i64::from(self.window_bounds.y) + i64::from(frame_rect.y)
            - i64::from(self.frame_viewport.y);
        let right = x + i64::from(frame_rect.width);
        let bottom = y + i64::from(frame_rect.height);
        let center_x = x + i64::from(frame_rect.width / 2);
        let center_y = y + i64::from(frame_rect.height / 2);
        if x < i64::from(i32::MIN)
            || y < i64::from(i32::MIN)
            || right > i64::from(i32::MAX)
            || bottom > i64::from(i32::MAX)
            || center_x < i64::from(self.window_bounds.x)
            || center_y < i64::from(self.window_bounds.y)
            || center_x >= self.window_bounds.right()
            || center_y >= self.window_bounds.bottom()
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
        })
    }
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
