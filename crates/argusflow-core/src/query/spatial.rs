//! AQL 空间查询使用的强类型锚点、方向与距离契约。

use std::fmt;

use serde::{Deserialize, Serialize};

use super::QueryExpr;

/// `nearest` 使用的空间锚点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpatialAnchor {
    /// 由查询唯一解析出的元素锚点。
    Element {
        /// 必须唯一命中的锚点查询。
        query: Box<QueryExpr>,
    },
    /// 当前视觉窗口的一个角。
    ViewportCorner {
        /// 用作距离原点的窗口角。
        position: ViewportCorner,
    },
    /// 当前视觉窗口的一条边。
    ViewportEdge {
        /// 用作距离原点的窗口边。
        side: ViewportEdge,
    },
}

/// 视觉 viewport 的四个语义角。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportCorner {
    /// 左上角。
    TopLeft,
    /// 右上角。
    TopRight,
    /// 左下角。
    BottomLeft,
    /// 右下角。
    BottomRight,
}

impl fmt::Display for ViewportCorner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        })
    }
}

/// 视觉 viewport 的四条语义边。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportEdge {
    /// 上边。
    Top,
    /// 右边。
    Right,
    /// 下边。
    Bottom,
    /// 左边。
    Left,
}

impl fmt::Display for ViewportEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        })
    }
}

/// 空间查询相对于元素 anchor 的有限方向集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialDirection {
    /// 不限制方向。
    Any,
    /// 目标位于锚点上方。
    Above,
    /// 目标位于锚点下方。
    Below,
    /// 目标位于锚点左侧。
    Left,
    /// 目标位于锚点右侧。
    Right,
}

impl fmt::Display for SpatialDirection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Any => "any",
            Self::Above => "above",
            Self::Below => "below",
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

/// 视觉空间排序使用的分辨率无关距离度量。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// 矩形边缘间隙分别按 viewport 宽高归一化。
    #[default]
    EdgeGapNormalized,
    /// 矩形中心距离分别按 viewport 宽高归一化。
    CenterDistanceNormalized,
}

impl fmt::Display for DistanceMetric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EdgeGapNormalized => "edge_gap",
            Self::CenterDistanceNormalized => "center_distance",
        })
    }
}
