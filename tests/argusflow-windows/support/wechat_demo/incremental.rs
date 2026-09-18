//! 差分规划和文本缓存合并；检测与识别均仅消费规划出的变化小图。
use super::{
    frame::{Frame, union},
    text::TextRecord,
};
use argusflow_capture_contracts::PixelRect;
use argusflow_core::Operation;
use argusflow_image::{DifferencePolicy, ImageView, changed_regions};
use std::error::Error;

/// 本轮识别范围的原因，便于核对真实增量效果。
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateKind {
    /// 尚未有基线。
    Initial,
    /// 窗口尺寸改变，旧几何全部作废。
    Resized,
    /// 变化范围太大或太碎，重新建立完整快照。
    BroadChange,
    /// 只检测/识别局部变化。
    Incremental,
    /// 像素未变化，没有调用模型。
    Unchanged,
}
/// 非重叠的识别区域，均使用全窗口局部物理像素。
pub struct UpdatePlan {
    /// 规划原因。
    pub kind: UpdateKind,
    /// 需要送入 OCR 的小图范围。
    pub regions: Vec<PixelRect>,
}
/// 不修改基线；所有小图 OCR 成功后才能提交新帧与合并结果。
pub fn plan(
    previous: Option<&Frame>,
    current: &Frame,
    blocks: &[TextRecord],
    operation: &Operation,
) -> Result<UpdatePlan, Box<dyn Error>> {
    let bounds = current.bounds.local();
    let Some(previous) = previous else {
        return Ok(full(bounds, UpdateKind::Initial));
    };
    if previous.bounds.local() != bounds {
        return Ok(full(bounds, UpdateKind::Resized));
    }
    let changes = changed_regions(
        ImageView::triples(bounds.width(), bounds.height(), &previous.pixels)?,
        ImageView::triples(bounds.width(), bounds.height(), &current.pixels)?,
        DifferencePolicy::default(),
        operation,
    )?;
    if changes.is_empty() {
        return Ok(UpdatePlan {
            kind: UpdateKind::Unchanged,
            regions: Vec::new(),
        });
    }
    if changes.len() > 4096 {
        return Ok(full(bounds, UpdateKind::BroadChange));
    }
    // 外扩保留字形上下文，合并后继续吸收被切到的旧文本块，避免半行文字和幽灵缓存。
    let mut regions: Vec<_> = changes
        .into_iter()
        .filter_map(|c| c.bounds.expanded(24, bounds))
        .collect();
    loop {
        operation.check("demo_dirty_regions")?;
        coalesce(&mut regions)?;
        let mut expanded = false;
        for region in &mut regions {
            for block in blocks {
                if region.intersection(block.rect).is_some() && !region.contains(block.rect) {
                    *region = union(*region, block.rect)?
                        .expanded(8, bounds)
                        .ok_or("变化区域扩展失败")?;
                    expanded = true;
                }
            }
        }
        if !expanded {
            break;
        }
    }
    let pixels: u64 = regions
        .iter()
        .map(|r| u64::from(r.width()) * u64::from(r.height()))
        .sum();
    if regions.len() > 24
        || pixels * 100 >= u64::from(bounds.width()) * u64::from(bounds.height()) * 65
    {
        return Ok(full(bounds, UpdateKind::BroadChange));
    }
    regions.sort_by_key(|r| (r.y(), r.x()));
    Ok(UpdatePlan {
        kind: UpdateKind::Incremental,
        regions,
    })
}
fn full(bounds: PixelRect, kind: UpdateKind) -> UpdatePlan {
    UpdatePlan {
        kind,
        regions: vec![bounds],
    }
}
fn coalesce(regions: &mut Vec<PixelRect>) -> Result<(), Box<dyn Error>> {
    let mut i = 0;
    while i < regions.len() {
        let mut j = i + 1;
        while j < regions.len() {
            if regions[i].intersection(regions[j]).is_some() {
                regions[i] = union(regions[i], regions.remove(j))?;
                // 扩展后的并集可能重新碰到之前检查过的区域。
                j = i + 1;
            } else {
                j += 1;
            }
        }
        i += 1;
    }
    // 后面的合并可能与更早区域相交；继续直到两两不相交。
    if regions.iter().enumerate().any(|(i, a)| {
        regions[i + 1..]
            .iter()
            .any(|b| a.intersection(*b).is_some())
    }) {
        coalesce(regions)?;
    }
    Ok(())
}
/// 覆盖区中的旧文字即使没有新文字替换也必须删除；未变区域保留。
pub fn merge(
    previous: &[TextRecord],
    plan: &UpdatePlan,
    fresh: Vec<TextRecord>,
) -> Vec<TextRecord> {
    let mut blocks: Vec<_> = match plan.kind {
        UpdateKind::Initial | UpdateKind::Resized | UpdateKind::BroadChange => Vec::new(),
        UpdateKind::Incremental | UpdateKind::Unchanged => previous
            .iter()
            .filter(|b| {
                !plan
                    .regions
                    .iter()
                    .any(|r| r.intersection(b.rect).is_some())
            })
            .cloned()
            .collect(),
    };
    blocks.extend(fresh);
    blocks.sort_by_key(|b| (b.rect.y(), b.rect.x()));
    blocks
}
