//! 校验后的物理像素几何，所有矩形使用半开区间。
use crate::{CaptureError, CaptureResult};
use argusflow_core::FailureKind;

/// 图像内部非空整数矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PixelRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
impl PixelRect {
    /// 拒绝空区域及坐标溢出。
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> CaptureResult<Self> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "rectangle",
                "像素区域为空或溢出",
            ));
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    /// 左边界。
    pub fn x(self) -> u32 {
        self.x
    }
    /// 上边界。
    pub fn y(self) -> u32 {
        self.y
    }
    /// 宽度。
    pub fn width(self) -> u32 {
        self.width
    }
    /// 高度。
    pub fn height(self) -> u32 {
        self.height
    }
    /// 右侧开边界。
    pub fn right(self) -> u32 {
        self.x + self.width
    }
    /// 下侧开边界。
    pub fn bottom(self) -> u32 {
        self.y + self.height
    }
    /// 四通道紧密布局字节数。
    pub fn byte_len(self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * 4
    }
    /// 是否完全覆盖另一个矩形。
    pub fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }
    /// 求交；空交集不构造无效矩形。
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (x < right && y < bottom).then(|| Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
    /// 扩展后裁到图像范围；不会溢出。
    pub fn expanded(self, padding: u32, bounds: Self) -> Option<Self> {
        let x = self.x.saturating_sub(padding).max(bounds.x);
        let y = self.y.saturating_sub(padding).max(bounds.y);
        let right = self.right().saturating_add(padding).min(bounds.right());
        let bottom = self.bottom().saturating_add(padding).min(bounds.bottom());
        (x < right && y < bottom).then(|| Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

/// 虚拟桌面物理像素范围，允许负原点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
impl ScreenRect {
    /// 拒绝空尺寸及超出 i32 坐标空间的右下边界。
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> CaptureResult<Self> {
        PixelRect::new(0, 0, width, height)?;
        if i64::from(x) + i64::from(width) > i64::from(i32::MAX)
            || i64::from(y) + i64::from(height) > i64::from(i32::MAX)
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "screen_rectangle",
                "屏幕区域坐标溢出",
            ));
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    /// 屏幕横坐标。
    pub fn x(self) -> i32 {
        self.x
    }
    /// 屏幕纵坐标。
    pub fn y(self) -> i32 {
        self.y
    }
    /// 宽度。
    pub fn width(self) -> u32 {
        self.width
    }
    /// 高度。
    pub fn height(self) -> u32 {
        self.height
    }
    /// 对应的图像本地边界。
    pub fn local(self) -> PixelRect {
        PixelRect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        }
    }
    /// 将图像本地区域转换为虚拟桌面坐标。
    pub fn project(self, region: PixelRect) -> CaptureResult<Self> {
        if !self.local().contains(region) {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "project",
                "区域超出屏幕",
            ));
        }
        Self::new(
            (i64::from(self.x) + i64::from(region.x)) as i32,
            (i64::from(self.y) + i64::from(region.y)) as i32,
            region.width,
            region.height,
        )
    }
}

/// 原始输出纹理转为屏幕方向所需的顺时针旋转。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rotation {
    /// 无旋转。
    Identity,
    /// 顺时针九十度。
    Clockwise90,
    /// 一百八十度。
    Clockwise180,
    /// 顺时针二百七十度。
    Clockwise270,
}
