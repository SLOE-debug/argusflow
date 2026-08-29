//! VisionQueryPlan 在 VisualSceneIndex 上的空间执行。

use argusflow_core::DistanceMetric;
use thiserror::Error;

use crate::{
    index::{
        VisualSceneSnapshot, center_distance_normalized, direction_matches, edge_gap_normalized,
    },
    ocr::normalize_text,
    scene::VisualNode,
};

use super::{VisionPlanExpr, VisionQueryPlan, VisionTextPredicate};

/// Vision AQL 执行无法给出安全的确定结果。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum VisionQueryExecutionError {
    /// 全局查询面对未完整观测或仍 dirty 的 scene。
    #[error("visual observation is incomplete or contains unrefreshed dirty regions")]
    ObservationIncomplete,
    /// nearest anchor 没有命中。
    #[error("nearest anchor was not found")]
    AnchorNotFound,
    /// nearest anchor 命中多个节点。
    #[error("nearest anchor is ambiguous across {matches} nodes")]
    AnchorAmbiguous {
        /// anchor 候选数。
        matches: usize,
    },
    /// 查询或显式 rank 没有候选。
    #[error("visual query target was not found")]
    TargetNotFound,
    /// 普通查询或距离 rank 仍包含多个等价候选。
    #[error("visual query target is ambiguous across {matches} nodes")]
    TargetAmbiguous {
        /// 等价候选数。
        matches: usize,
    },
}

/// Vision 查询执行的候选与空间 Explain。
#[derive(Debug)]
pub struct VisionQueryResult<'scene> {
    /// 当前 scene 中有序候选。
    pub matches: Vec<&'scene VisualNode>,
    /// 实际执行步骤及候选数量。
    pub explain: Vec<String>,
}

/// 在一致性 snapshot 上运行冻结 Vision 计划。
pub fn execute_vision_query<'scene>(
    snapshot: &'scene VisualSceneSnapshot,
    plan: &VisionQueryPlan,
) -> Result<VisionQueryResult<'scene>, VisionQueryExecutionError> {
    if plan.needs_complete_scene
        && (!snapshot.observation.coverage.is_complete()
            || !snapshot.observation.dirty_regions.is_empty())
    {
        return Err(VisionQueryExecutionError::ObservationIncomplete);
    }
    let matches = evaluate(snapshot, &plan.root)?;
    let mut explain = plan.summary.clone();
    explain.push(format!("result candidates: {}", matches.len()));
    Ok(VisionQueryResult { matches, explain })
}

/// 执行计划并要求最终 0/1/N 语义收敛为唯一节点。
pub fn execute_unique_vision_query<'scene>(
    snapshot: &'scene VisualSceneSnapshot,
    plan: &VisionQueryPlan,
) -> Result<&'scene VisualNode, VisionQueryExecutionError> {
    let result = execute_vision_query(snapshot, plan)?;
    match result.matches.as_slice() {
        [] => Err(VisionQueryExecutionError::TargetNotFound),
        [node] => Ok(*node),
        nodes => Err(VisionQueryExecutionError::TargetAmbiguous {
            matches: nodes.len(),
        }),
    }
}

/// 递归执行 Vision IR。
fn evaluate<'scene>(
    snapshot: &'scene VisualSceneSnapshot,
    expression: &VisionPlanExpr,
) -> Result<Vec<&'scene VisualNode>, VisionQueryExecutionError> {
    Ok(match expression {
        VisionPlanExpr::TextLookup(predicate) => {
            let expected = match predicate {
                VisionTextPredicate::Exact(text) | VisionTextPredicate::Contains(text) => {
                    normalize_text(text)
                }
            };
            if expected.is_empty() {
                Vec::new()
            } else {
                match predicate {
                    VisionTextPredicate::Exact(_) => snapshot.index.exact_text(&expected),
                    VisionTextPredicate::Contains(_) => snapshot.index.contains_text(&expected),
                }
            }
        }
        VisionPlanExpr::Any(branches) => {
            let mut selected = Vec::new();
            for branch in branches {
                let matches = evaluate(snapshot, branch)?;
                if !matches.is_empty() {
                    selected = matches;
                    break;
                }
            }
            selected
        }
        VisionPlanExpr::First(query) => evaluate(snapshot, query)?.into_iter().take(1).collect(),
        VisionPlanExpr::Nth { query, index } => evaluate(snapshot, query)?
            .into_iter()
            .nth(index.saturating_sub(1))
            .into_iter()
            .collect(),
        VisionPlanExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => {
            let anchors = evaluate(snapshot, anchor)?;
            let anchor = match anchors.as_slice() {
                [] => return Err(VisionQueryExecutionError::AnchorNotFound),
                [anchor] => *anchor,
                anchors => {
                    return Err(VisionQueryExecutionError::AnchorAmbiguous {
                        matches: anchors.len(),
                    });
                }
            };
            let mut ranked = evaluate(snapshot, target)?
                .into_iter()
                .filter(|candidate| candidate.id != anchor.id)
                .filter(|candidate| direction_matches(anchor, candidate, *direction))
                .map(|candidate| {
                    let distance = match metric {
                        DistanceMetric::EdgeGapNormalized => edge_gap_normalized(
                            anchor.bbox,
                            candidate.bbox,
                            snapshot.scene.viewport,
                        ),
                        DistanceMetric::CenterDistanceNormalized => center_distance_normalized(
                            anchor.bbox,
                            candidate.bbox,
                            snapshot.scene.viewport,
                        ),
                    };
                    (distance, candidate)
                })
                .collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.id.cmp(&right.1.id))
            });
            select_distance_rank(&ranked, *index)?
        }
    })
}

/// 按 epsilon 分组并选择显式距离 rank，tie 不用 NodeId 偷偷裁决。
fn select_distance_rank<'scene>(
    ranked: &[(f32, &'scene VisualNode)],
    selected_rank: usize,
) -> Result<Vec<&'scene VisualNode>, VisionQueryExecutionError> {
    const DISTANCE_EPSILON: f32 = 1.0e-6;
    let mut groups: Vec<Vec<&VisualNode>> = Vec::new();
    let mut last_distance: Option<f32> = None;
    for (distance, node) in ranked {
        if last_distance.is_none_or(|last| (distance - last).abs() > DISTANCE_EPSILON) {
            groups.push(Vec::new());
            last_distance = Some(*distance);
        }
        groups
            .last_mut()
            .expect("group was just created")
            .push(*node);
    }
    let Some(group) = groups.get(selected_rank.saturating_sub(1)) else {
        return Err(VisionQueryExecutionError::TargetNotFound);
    };
    if group.len() > 1 {
        return Err(VisionQueryExecutionError::TargetAmbiguous {
            matches: group.len(),
        });
    }
    Ok(group.clone())
}
