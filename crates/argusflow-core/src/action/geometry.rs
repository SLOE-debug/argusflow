//! 屏幕物理像素、网页 CSS 像素和图片坐标不可混用。

use crate::{Failure, FailureKind};

/// 虚拟桌面的物理像素坐标，允许负数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPoint {
    /// 从虚拟桌面原点计算的横坐标。
    pub x: i32,
    /// 从虚拟桌面原点计算的纵坐标。
    pub y: i32,
}

/// 主文档视口内的 CSS 像素坐标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPoint {
    x: f64,
    y: f64,
}

impl CssPoint {
    /// 拒绝非有限值和视口外的负数。
    pub fn new(x: f64, y: f64) -> Result<Self, Failure> {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "css_point",
                "坐标必须是非负有限数",
            ));
        }
        Ok(Self { x, y })
    }
    /// CSS 横坐标。
    pub fn x(self) -> f64 {
        self.x
    }
    /// CSS 纵坐标。
    pub fn y(self) -> f64 {
        self.y
    }
}

/// 原图左上角为原点的像素坐标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImagePoint {
    /// 原图横坐标。
    pub x: f32,
    /// 原图纵坐标。
    pub y: f32,
}
