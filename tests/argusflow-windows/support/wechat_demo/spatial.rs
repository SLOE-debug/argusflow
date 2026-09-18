//! 不依赖微信或 OCR 引擎的空间排序；序号从 1 开始。
use argusflow_core::ScreenPoint;
use std::num::NonZeroUsize;

/// OCR 文本中心在物理屏幕上的定位候选。
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// 完整识别文字，不修改或归一化。
    pub text: String,
    /// 识别四边形中心的物理屏幕坐标。
    pub point: ScreenPoint,
    /// 本次 OCR 结果中的块索引，用于稳定消歧。
    pub index: usize,
}
/// 相对锚点的方向，横向要求同行，纵向要求同列。
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    /// 锚点左侧且纵向差值不超过容差。
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "demo 流程只用向右和向下，向左由空间契约测试覆盖")
    )]
    Left,
    /// 锚点右侧且纵向差值不超过容差。
    Right,
    /// 锚点上方且横向差值不超过容差。
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "demo 流程只用向右和向下，向上由空间契约测试覆盖")
    )]
    Above,
    /// 锚点下方且横向差值不超过容差。
    Below,
}

/// 最靠顶部，同行时最靠左。调用方应先限制搜索区域。
pub fn top_left(candidates: &[Candidate]) -> Option<&Candidate> {
    candidates
        .iter()
        .min_by_key(|c| (c.point.y, c.point.x, c.index))
}

/// 在方向及垂直于方向的容差内，按中心欧氏距离取第 N 个。
pub fn nearest<'a>(
    candidates: &'a [Candidate],
    anchor: &Candidate,
    direction: Direction,
    nth: NonZeroUsize,
    tolerance: u32,
) -> Option<&'a Candidate> {
    let mut ranked: Vec<_> = candidates
        .iter()
        .filter_map(|candidate| {
            let dx = i64::from(candidate.point.x) - i64::from(anchor.point.x);
            let dy = i64::from(candidate.point.y) - i64::from(anchor.point.y);
            let aligned = match direction {
                Direction::Left => dx < 0 && dy.unsigned_abs() <= u64::from(tolerance),
                Direction::Right => dx > 0 && dy.unsigned_abs() <= u64::from(tolerance),
                Direction::Above => dy < 0 && dx.unsigned_abs() <= u64::from(tolerance),
                Direction::Below => dy > 0 && dx.unsigned_abs() <= u64::from(tolerance),
            };
            // u128 可容纳两个完整 i32 坐标差的平方和。
            aligned.then_some((
                u128::from(dx.unsigned_abs()).pow(2) + u128::from(dy.unsigned_abs()).pow(2),
                candidate,
            ))
        })
        .collect();
    ranked.sort_by_key(|(distance, c)| (*distance, c.point.y, c.point.x, c.index));
    ranked.get(nth.get() - 1).map(|(_, candidate)| *candidate)
}
