//! 捕获帧的身份、坐标空间和像素格式契约。

use serde::{Deserialize, Serialize};

/// 单次捕获流中的单调帧标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameId(u64);

impl FrameId {
    /// 从捕获实现提供的单调值构造帧标识。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回不透明标识的数值表示。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 窗口拓扑变化代数；拓扑变化会使旧场景失效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TopologyGeneration(u64);

impl TopologyGeneration {
    /// 创建指定数值的拓扑代数。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回不透明代数的数值表示。
    pub const fn get(self) -> u64 {
        self.0
    }

    /// 判断调用方是否尚未提供具体拓扑代数。
    pub const fn is_unknown(self) -> bool {
        self.0 == 0
    }
}

/// 捕获实现使用的高精度时钟刻度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QpcTimestamp(u64);

impl QpcTimestamp {
    /// 创建指定数值的时间戳。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 返回时间戳数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 视觉管线内部统一使用的物理像素矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhysicalRect {
    /// 左上角水平坐标，单位为物理像素。
    pub x: i32,
    /// 左上角垂直坐标，单位为物理像素。
    pub y: i32,
    /// 矩形宽度，单位为物理像素。
    pub width: u32,
    /// 矩形高度，单位为物理像素。
    pub height: u32,
}

impl PhysicalRect {
    /// 创建一个非空物理矩形。
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// 返回右边界（半开区间）。
    pub fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    /// 返回下边界（半开区间）。
    pub fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    /// 返回矩形面积。
    pub const fn area(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// 判断另一个矩形是否与当前矩形相交。
    pub fn intersects(self, other: Self) -> bool {
        i64::from(self.x) < other.right()
            && i64::from(other.x) < self.right()
            && i64::from(self.y) < other.bottom()
            && i64::from(other.y) < self.bottom()
    }

    /// 判断两个矩形是否相交或共享边界，供 tile merge 使用。
    pub fn touches(self, other: Self) -> bool {
        i64::from(self.x) <= other.right()
            && i64::from(other.x) <= self.right()
            && i64::from(self.y) <= other.bottom()
            && i64::from(other.y) <= self.bottom()
    }

    /// 返回两个矩形的包围盒。
    pub fn union(self, other: Self) -> Self {
        let left = i64::from(self.x).min(i64::from(other.x));
        let top = i64::from(self.y).min(i64::from(other.y));
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self {
            x: left as i32,
            y: top as i32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        }
    }

    /// 将矩形向外扩展并限制到一个边界矩形内。
    pub fn expand_clamped(self, padding: u32, bounds: Self) -> Self {
        let left = i64::from(self.x) - i64::from(padding);
        let top = i64::from(self.y) - i64::from(padding);
        let right = self.right() + i64::from(padding);
        let bottom = self.bottom() + i64::from(padding);
        let bound_left = i64::from(bounds.x);
        let bound_top = i64::from(bounds.y);
        let bound_right = bounds.right();
        let bound_bottom = bounds.bottom();
        let clipped_left = left.max(bound_left);
        let clipped_top = top.max(bound_top);
        let clipped_right = right.min(bound_right);
        let clipped_bottom = bottom.min(bound_bottom);
        Self {
            x: clipped_left as i32,
            y: clipped_top as i32,
            width: (clipped_right - clipped_left).max(1) as u32,
            height: (clipped_bottom - clipped_top).max(1) as u32,
        }
    }

    /// 判断当前矩形是否完整位于另一个矩形内。
    pub fn is_inside(self, bounds: Self) -> bool {
        i64::from(self.x) >= i64::from(bounds.x)
            && i64::from(self.y) >= i64::from(bounds.y)
            && self.right() <= bounds.right()
            && self.bottom() <= bounds.bottom()
    }
}

/// bbox 或 polygon 使用的坐标语义，避免跨 DPI 混用数值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    /// 相对于捕获帧左上角的物理像素。
    FrameLocal,
    /// 相对于窗口 client area 左上角的物理像素。
    ClientPhysical,
    /// 相对于 Windows virtual screen 左上角的物理像素。
    VirtualScreenPhysical,
}

/// 当前捕获实现输出的像素格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// 每个像素四字节，顺序为蓝、绿、红、透明。
    Bgra8Unorm,
}

impl PixelFormat {
    /// 返回单像素字节数。
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra8Unorm => 4,
        }
    }
}
