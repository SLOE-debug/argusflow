//! Windows 输入前的旧视觉查询与 AQL Vision 查询统一选择。

use argusflow_core::{AutomationError, BackendKind, PreparedTargetLocator};
use argusflow_vision::{
    ObservationCoverage, ObservationState, PreparedVisionQuery, VisionQueryExecutionError,
    VisualNode, VisualQueryReport, VisualScene, VisualSceneSnapshot, execute_unique_vision_query,
    matching_nodes,
};
use std::sync::Arc;

/// 视觉点击必须先通过置信度门槛，低置信度候选只能触发后续升级阶段。
const MIN_CLICK_CONFIDENCE: f32 = 0.80;

/// 将 Runtime 冻结定位器编译为 Vision 可执行查询。
pub(super) fn prepare_query(
    locator: &PreparedTargetLocator,
) -> Result<PreparedVisionQuery, AutomationError> {
    match locator {
        PreparedTargetLocator::Visual { query } => Ok(PreparedVisionQuery::Legacy(query.clone())),
        PreparedTargetLocator::Query { query, parameters } => {
            PreparedVisionQuery::from_aql(query, parameters).map_err(|message| {
                AutomationError::BackendFailed {
                    backend: BackendKind::VisualCache,
                    message,
                }
            })
        }
        PreparedTargetLocator::Coordinate { .. } | PreparedTargetLocator::Focused => {
            Err(AutomationError::BackendUnavailable {
                backend: BackendKind::VisualCache,
                message: "target locator cannot be compiled as a visual scene query".to_owned(),
            })
        }
    }
}

/// 只接受唯一且达到置信度门槛的节点；其余结果留给后续升级阶段。
pub(super) fn select_click_node<'scene>(
    scene: &'scene Arc<VisualScene>,
    query: &PreparedVisionQuery,
) -> Result<&'scene VisualNode, AutomationError> {
    let node = match query {
        PreparedVisionQuery::Legacy(query) => {
            let candidates = matching_nodes(scene, query);
            let report = VisualQueryReport::from_matches(scene, query, &candidates);
            match candidates.as_slice() {
                [] => {
                    return Err(AutomationError::TargetNotFound {
                        query: query.text.clone(),
                        details: format!("；{}", report.summary()),
                    });
                }
                [node] => *node,
                candidates => {
                    return Err(AutomationError::AmbiguousTarget {
                        query: query.text.clone(),
                        matches: candidates.len(),
                        details: format!("；{}", report.summary()),
                    });
                }
            }
        }
        PreparedVisionQuery::Aql { source, plan } => {
            // Scene 只会从整窗 cache lookup 或完成 dirty refresh 的 runtime 路径进入输入层；
            // snapshot 仍显式携带 Complete，防止 executor 把空节点误认为未观测。
            let snapshot = VisualSceneSnapshot::new(
                scene.clone(),
                ObservationState {
                    coverage: ObservationCoverage::Complete,
                    fresh_regions: Vec::new(),
                    dirty_regions: Vec::new(),
                },
            );
            execute_unique_vision_query(&snapshot, plan)
                .map_err(|error| map_query_error(source, error))?
                .to_owned_ref(scene)
                .ok_or_else(|| AutomationError::VisualTargetStale {
                    message: "AQL result no longer belongs to the selected visual scene".to_owned(),
                })?
        }
    };
    if node.confidence < MIN_CLICK_CONFIDENCE {
        return Err(AutomationError::TargetNotFound {
            query: query.source().to_owned(),
            details: format!(
                "visual candidate confidence {:.0}% is below click threshold",
                node.confidence * 100.0
            ),
        });
    }
    Ok(node)
}

/// 将 cloned snapshot 节点映射回原 scene 借用。
trait OriginalSceneNode {
    /// 按稳定 node ID 取回原 scene 节点。
    fn to_owned_ref<'scene>(&self, scene: &'scene VisualScene) -> Option<&'scene VisualNode>;
}

impl OriginalSceneNode for VisualNode {
    fn to_owned_ref<'scene>(&self, scene: &'scene VisualScene) -> Option<&'scene VisualNode> {
        scene.nodes.iter().find(|node| node.id == self.id)
    }
}

/// 将 Vision 空间错误保持为统一自动化 0/1/N 错误。
fn map_query_error(source: &str, error: VisionQueryExecutionError) -> AutomationError {
    match error {
        VisionQueryExecutionError::TargetNotFound | VisionQueryExecutionError::AnchorNotFound => {
            AutomationError::TargetNotFound {
                query: source.to_owned(),
                details: error.to_string(),
            }
        }
        VisionQueryExecutionError::TargetAmbiguous { matches }
        | VisionQueryExecutionError::AnchorAmbiguous { matches } => {
            AutomationError::AmbiguousTarget {
                query: source.to_owned(),
                matches,
                details: error.to_string(),
            }
        }
        VisionQueryExecutionError::ObservationIncomplete => AutomationError::BackendFailed {
            backend: BackendKind::VisualCache,
            message: error.to_string(),
        },
    }
}
