//! 动作前后必须保持唯一且位置连续的视觉上下文。

use std::sync::Arc;

use argusflow_core::AutomationError;

use super::FrozenVisionQuery;
use crate::{PhysicalRect, VisualScene};

/// 一条上下文查询及其动作前唯一命中的位置。
#[derive(Debug)]
pub(super) struct StableContextSnapshot {
    /// 动态参数已冻结的 AQL 查询计划。
    query: FrozenVisionQuery,
    /// 动作前唯一实例的帧内位置。
    bounds: PhysicalRect,
}

/// 在动作前要求每条上下文查询都严格唯一，避免向错误会话提交输入。
pub(super) fn capture_stable_context(
    scene: &Arc<VisualScene>,
    queries: Vec<FrozenVisionQuery>,
) -> Result<Vec<StableContextSnapshot>, AutomationError> {
    queries
        .into_iter()
        .enumerate()
        .map(|(index, query)| {
            let matches = query.evaluate(scene)?;
            match matches.as_slice() {
                [candidate] => Ok(StableContextSnapshot {
                    query,
                    bounds: candidate.bbox,
                }),
                [] => Err(AutomationError::TargetNotFound {
                    query: format!("visual stable context #{}", index + 1),
                    details: " was not present in the frozen action window".to_owned(),
                }),
                candidates => Err(AutomationError::AmbiguousTarget {
                    query: format!("visual stable context #{}", index + 1),
                    matches: candidates.len(),
                    details: " in the frozen action window".to_owned(),
                }),
            }
        })
        .collect()
}

/// 确保动作后的上下文仍严格唯一，并与动作前事实位置相交。
pub(super) fn stable_context_preserved(
    expected: &[StableContextSnapshot],
    scene: &Arc<VisualScene>,
) -> Result<(), String> {
    for (index, snapshot) in expected.iter().enumerate() {
        let matches = snapshot
            .query
            .evaluate(scene)
            .map_err(|error| format!("动作后上下文 #{} 求值失败：{error}", index + 1))?;
        let [candidate] = matches.as_slice() else {
            return Err(format!(
                "动作后上下文 #{} 不再严格唯一（命中 {} 项）",
                index + 1,
                matches.len(),
            ));
        };
        if !candidate.bbox.intersects(snapshot.bounds) {
            return Err(format!("动作后上下文 #{} 已离开动作前位置", index + 1));
        }
    }
    Ok(())
}
