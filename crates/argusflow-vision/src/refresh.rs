//! DirtyMap 到 OCR 刷新范围的显式规划。

use serde::{Deserialize, Serialize};

use crate::{diff::DirtyMap, frame::PhysicalRect};

/// 一次刷新计划的原因，供 metrics、Explain 和 evidence 关联。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshReason {
    /// 作用域没有可复用的基础场景。
    NoBaseScene,
    /// 缓存超过本次调用允许的 freshness。
    CacheExpired,
    /// 当前查询区域与 dirty 区域没有交集。
    QueryOutsideDirty,
    /// 变化范围足够小，可以只刷新若干局部区域。
    DirtyRegion,
    /// 拓扑或画面发生大范围变化。
    MajorTransition,
    /// 调用方明确要求完整刷新。
    ExplicitFull,
    /// cache 已覆盖当前查询且仍在 freshness 预算内。
    CacheValid,
}

/// Runtime 交给 OCR 执行器的最小刷新计划。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RefreshPlan {
    /// 只读取当前作用域缓存，不捕获或调用 OCR。
    CacheOnly {
        /// 不刷新时仍需记录的原因。
        reason: RefreshReason,
    },
    /// 只重新识别这些不相交或已合并的物理区域。
    Partial {
        /// 本次 OCR 必须覆盖的区域。
        regions: Vec<PhysicalRect>,
        /// 这些区域占当前 viewport 的面积比例。
        coverage_ratio: f32,
        /// 形成局部计划的原因。
        reason: RefreshReason,
    },
    /// 重新识别完整 viewport。
    Full {
        /// 升级完整刷新的原因。
        reason: RefreshReason,
    },
}

impl RefreshPlan {
    /// 返回计划是否会扫描完整 viewport。
    pub const fn is_full(&self) -> bool {
        matches!(self, Self::Full { .. })
    }

    /// 返回局部 OCR 区域；缓存或完整刷新没有局部区域。
    pub fn regions(&self) -> &[PhysicalRect] {
        match self {
            Self::Partial { regions, .. } => regions,
            Self::CacheOnly { .. } | Self::Full { .. } => &[],
        }
    }
}

/// 根据 dirty map、查询 ROI 和 cache 状态选择本次最小刷新范围。
pub fn choose_refresh_plan(
    dirty: Option<&DirtyMap>,
    viewport: PhysicalRect,
    query_region: Option<PhysicalRect>,
    has_base_scene: bool,
    cache_expired: bool,
    force_full: bool,
    full_refresh_dirty_ratio: f32,
) -> RefreshPlan {
    if force_full {
        return RefreshPlan::Full {
            reason: RefreshReason::ExplicitFull,
        };
    }
    if !has_base_scene {
        return RefreshPlan::Full {
            reason: RefreshReason::NoBaseScene,
        };
    }
    let Some(dirty) = dirty else {
        return RefreshPlan::Full {
            reason: RefreshReason::MajorTransition,
        };
    };
    if dirty.major_transition {
        return RefreshPlan::Full {
            reason: RefreshReason::MajorTransition,
        };
    }

    let regions = dirty
        .regions
        .iter()
        .filter_map(|region| intersect(region.rect, query_region.unwrap_or(viewport)))
        .collect::<Vec<_>>();
    if regions.is_empty() {
        return if cache_expired {
            RefreshPlan::Partial {
                regions: vec![query_region.unwrap_or(viewport)],
                coverage_ratio: coverage_ratio(&[query_region.unwrap_or(viewport)], viewport),
                reason: RefreshReason::CacheExpired,
            }
        } else {
            RefreshPlan::CacheOnly {
                reason: RefreshReason::QueryOutsideDirty,
            }
        };
    }

    let coverage = coverage_ratio(&regions, viewport);
    if coverage >= full_refresh_dirty_ratio {
        RefreshPlan::Full {
            reason: RefreshReason::MajorTransition,
        }
    } else {
        RefreshPlan::Partial {
            regions,
            coverage_ratio: coverage,
            reason: RefreshReason::DirtyRegion,
        }
    }
}

/// 返回两个矩形的交集；差分区域已经在 frame viewport 内，但查询 ROI 可能更小。
fn intersect(left: PhysicalRect, right: PhysicalRect) -> Option<PhysicalRect> {
    let x = i64::from(left.x).max(i64::from(right.x));
    let y = i64::from(left.y).max(i64::from(right.y));
    let right_edge = left.right().min(right.right());
    let bottom_edge = left.bottom().min(right.bottom());
    let width = u32::try_from(right_edge - x).ok()?;
    let height = u32::try_from(bottom_edge - y).ok()?;
    PhysicalRect::new(
        i32::try_from(x).ok()?,
        i32::try_from(y).ok()?,
        width,
        height,
    )
}

/// 以合并前区域面积估算 OCR 覆盖率；重叠区域不会重复计入。
fn coverage_ratio(regions: &[PhysicalRect], viewport: PhysicalRect) -> f32 {
    let mut merged = Vec::new();
    for region in regions {
        let mut pending = *region;
        let mut merged_any = false;
        for existing in &mut merged {
            if existing.touches(pending) {
                *existing = existing.union(pending);
                merged_any = true;
                break;
            }
        }
        if !merged_any {
            merged.push(pending);
        }
    }
    let area = merged.iter().map(|region| region.area()).sum::<u64>();
    if viewport.area() == 0 {
        1.0
    } else {
        (area as f32 / viewport.area() as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diff::DirtyRegion, frame::FrameId};

    fn viewport() -> PhysicalRect {
        PhysicalRect::new(0, 0, 100, 100).unwrap()
    }

    fn dirty(rect: PhysicalRect, major_transition: bool) -> DirtyMap {
        DirtyMap {
            frame_id: FrameId::new(2),
            changed_area_ratio: if major_transition { 1.0 } else { 0.01 },
            compared_samples: 100,
            changed_samples: 1,
            major_transition,
            regions: vec![DirtyRegion {
                rect,
                changed_ratio: 0.1,
                reason: crate::diff::DirtyRegionReason::PixelDifference,
            }],
        }
    }

    #[test]
    fn outside_dirty_query_is_cache_only() {
        let plan = choose_refresh_plan(
            Some(&dirty(PhysicalRect::new(0, 0, 10, 10).unwrap(), false)),
            viewport(),
            Some(PhysicalRect::new(50, 50, 10, 10).unwrap()),
            true,
            false,
            false,
            0.35,
        );
        assert!(matches!(plan, RefreshPlan::CacheOnly { .. }));
    }

    #[test]
    fn major_transition_is_full() {
        let plan = choose_refresh_plan(
            Some(&dirty(viewport(), true)),
            viewport(),
            None,
            true,
            false,
            false,
            0.35,
        );
        assert!(matches!(plan, RefreshPlan::Full { .. }));
    }
}
