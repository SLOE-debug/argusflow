//! 空间条件不包含平台坐标或原生句柄。
use super::{Expr, Operand};

/// 长度所依据的统一单位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthUnit {
    /// 经 DPI 换算的逻辑像素。
    LogicalPixels,
    /// 范围短边百分比。
    ScopeShort,
    /// 范围宽度百分比。
    ScopeWidth,
    /// 范围高度百分比。
    ScopeHeight,
}
/// 数值允许由运行参数绑定，百分比使用 0..100 数值。
#[derive(Debug, Clone)]
pub struct Length {
    /// 非负数或类型化数值参数。
    pub value: Operand,
    /// 显式单位。
    pub unit: LengthUnit,
}
/// 明确的距离或位置排序；相同排序键不通过来源顺序偷偷消歧。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialOrder {
    /// 距离升序。
    Near,
    /// 距离降序。
    Far,
    /// 中心纵坐标升序。
    Top,
    /// 中心横坐标升序。
    Left,
    /// 先纵坐标、再横坐标升序。
    TopLeft,
}
/// 筛选和选择规则。
#[derive(Debug, Clone, Default)]
pub struct SpatialOptions {
    /// 逆时针半开角度区间。
    pub angles: Option<(f64, f64)>,
    /// 使用外框最短距离，默认中心距离。
    pub edge_distance: bool,
    /// 包含边界的距离下限。
    pub min_distance: Option<Length>,
    /// 包含边界的距离上限。
    pub max_distance: Option<Length>,
    /// true 为同一行，false 为同一列。
    pub row: Option<bool>,
    /// 中心对齐偏差上限。
    pub tolerance: Option<Length>,
    /// 排除与锚点相交的目标。
    pub exclude_overlap: bool,
    /// 目标必须与范围完全不相交。
    pub outside: bool,
    /// 使用两点包围矩形，默认线段带。
    pub between_rectangle: bool,
    /// 线段带全宽。
    pub bandwidth: Option<Length>,
    /// 主排序。
    pub order: Option<SpatialOrder>,
    /// 主排序相同时的次排序。
    pub secondary: Option<SpatialOrder>,
    /// 从一开始的选择序号。
    pub rank: Option<usize>,
}
/// 子查询可继续使用空间查询，区域与第二锚点必须明确且唯一。
#[derive(Debug, Clone)]
pub struct SpatialQuery {
    /// 必须唯一的锚点查询。
    pub anchor: Box<Expr>,
    /// 候选查询。
    pub target: Box<Expr>,
    /// 必须唯一的实际范围查询。
    pub region: Option<Box<Expr>>,
    /// 两锚点条件的另一个端点。
    pub second_anchor: Option<Box<Expr>>,
    /// 空间过滤与排序规则。
    pub options: SpatialOptions,
}
