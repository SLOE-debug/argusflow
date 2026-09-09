//! 只对请求范围汇总真实变化，订阅缺口保留为结果元数据。
use super::ProcessSummary;
use crate::{ChangeKind, normalize_regions, service::State, subtract_regions};
use argusflow_capture_contracts::*;

pub(crate) fn summarize(
    state: &State,
    source: SourceId,
    from: ClockTime,
    through: ClockTime,
    scope: PixelRect,
    ignored: &[PixelRect],
) -> CaptureResult<ProcessSummary> {
    let mut summary = ProcessSummary {
        last_change: from,
        ..Default::default()
    };
    if let Some(entry) = state.sources.get(&source)
        && from <= entry.log_floor
    {
        summary.gaps.push(Gap {
            source,
            from,
            through: entry.log_floor,
            reason: GapReason::ConsumerLagged,
        });
    }
    for record in &state.records {
        if record.source != source {
            continue;
        }
        match &record.kind {
            ChangeKind::Pixels { changes, .. } if record.time > from && record.time <= through => {
                let effective = subtract_regions(
                    &changes
                        .regions
                        .iter()
                        .filter_map(|rect| rect.intersection(scope))
                        .collect::<Vec<_>>(),
                    ignored,
                )?;
                if !effective.is_empty() {
                    summary.changes += 1;
                    summary.last_change = summary.last_change.max(record.time);
                    summary.regions.extend(effective);
                }
            }
            ChangeKind::Gap(gap) if gap.through > from && gap.from <= through => {
                summary.gaps.push(gap.clone())
            }
            _ => {}
        }
    }
    summary.regions = normalize_regions(&summary.regions)?;
    Ok(summary)
}
