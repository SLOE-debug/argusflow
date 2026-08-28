//! 视觉验证条件与高风险动作的三态结果。

use argusflow_core::VisualQuery;
use serde::{Deserialize, Serialize};

use crate::scene::{SceneId, VisualNode, VisualNodeId, VisualRegionId, VisualScene};

/// 当前视觉快照上可以安全解释的验证条件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VisualCondition {
    /// 指定区域内存在满足查询的文本。
    TextExists {
        /// 文本查询。
        query: VisualQuery,
        /// 可选区域；为空表示整个当前 viewport。
        region: Option<VisualRegionId>,
    },
    /// 自指定 scene 之后出现新的匹配文本。
    NewTextExistsSince {
        /// 文本查询。
        query: VisualQuery,
        /// 发送前或动作前的 scene ID。
        since_scene_id: SceneId,
        /// 可选区域；为空表示整个当前 viewport。
        region: Option<VisualRegionId>,
    },
}

/// 一次视觉验证的明确结果；`Uncertain` 不允许高风险流程自动继续。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    /// 已经有足够事实确认条件。
    Confirmed {
        /// 确认条件的节点 ID。
        matches: Vec<VisualNodeId>,
    },
    /// 当前稳定快照明确不满足条件。
    Rejected {
        /// 面向 Explain/Evidence 的原因。
        reason: String,
    },
    /// 缺少新快照、连续性或其它事实，不能安全下结论。
    Uncertain {
        /// 面向 Explain/Evidence 的原因。
        reason: String,
    },
}

impl VerificationOutcome {
    /// 判断验证是否允许继续后续高风险动作。
    pub const fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }
}

/// 在当前场景上评估一个视觉条件。
pub fn evaluate_visual_condition(
    current: Option<&VisualScene>,
    previous: Option<&VisualScene>,
    condition: &VisualCondition,
) -> VerificationOutcome {
    let Some(current) = current else {
        return VerificationOutcome::Uncertain {
            reason: "没有可用于验证的 current viewport scene".to_owned(),
        };
    };

    match condition {
        VisualCondition::TextExists { query, region } => {
            let matches = matching_nodes(current, query, *region);
            if matches.is_empty() {
                VerificationOutcome::Rejected {
                    reason: format!("当前 scene 中没有匹配文本：{}", query.text),
                }
            } else {
                VerificationOutcome::Confirmed {
                    matches: matches.into_iter().map(|node| node.id).collect(),
                }
            }
        }
        VisualCondition::NewTextExistsSince {
            query,
            since_scene_id,
            region,
        } => {
            if current.scene_id <= *since_scene_id {
                return VerificationOutcome::Uncertain {
                    reason: "current scene 尚未晚于验证起点 scene".to_owned(),
                };
            }
            let Some(previous) = previous else {
                return VerificationOutcome::Uncertain {
                    reason: "缺少验证起点的 previous scene，无法证明文本是新增的".to_owned(),
                };
            };
            if previous.scene_id != *since_scene_id {
                return VerificationOutcome::Uncertain {
                    reason: "previous scene 不是验证条件声明的起点 scene".to_owned(),
                };
            }
            if previous.window != current.window {
                return VerificationOutcome::Uncertain {
                    reason: "验证前后的 scene 不属于同一窗口".to_owned(),
                };
            }
            if !previous.topology_generation.is_unknown()
                && !current.topology_generation.is_unknown()
                && previous.topology_generation != current.topology_generation
            {
                return VerificationOutcome::Uncertain {
                    reason: "验证前后的 scene 属于不同窗口拓扑".to_owned(),
                };
            }
            let matches = matching_nodes(current, query, *region)
                .into_iter()
                .filter(|candidate| {
                    !previous.nodes.iter().any(|old| {
                        old.stable_hash == candidate.stable_hash
                            && old.normalized_text == candidate.normalized_text
                    })
                })
                .collect::<Vec<_>>();
            if matches.is_empty() {
                VerificationOutcome::Rejected {
                    reason: format!(
                        "自 scene {} 以来没有新增匹配文本：{}",
                        since_scene_id.get(),
                        query.text
                    ),
                }
            } else {
                VerificationOutcome::Confirmed {
                    matches: matches.into_iter().map(|node| node.id).collect(),
                }
            }
        }
    }
}

/// 返回满足文字与区域约束的节点，不会替调用方选择第一个候选。
fn matching_nodes<'scene>(
    scene: &'scene VisualScene,
    query: &VisualQuery,
    region: Option<VisualRegionId>,
) -> Vec<&'scene VisualNode> {
    let expected = crate::normalize_text(&query.text);
    if expected.is_empty() {
        return Vec::new();
    }
    scene
        .nodes
        .iter()
        .filter(|node| region.is_none_or(|region| node.region_id == Some(region)))
        .filter(|node| {
            if query.exact {
                node.normalized_text == expected
            } else {
                node.normalized_text.contains(&expected)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        frame::{FrameId, PhysicalRect, QpcTimestamp, TopologyGeneration},
        image::CapturedFrame,
        ocr::{OcrItem, OcrModel, OcrRequestId, OcrResponse, PolygonPoint},
        scene::{SceneBuildOptions, VisualSceneBuilder},
    };
    use argusflow_core::WindowIdentity;

    fn scenes() -> (VisualScene, VisualScene) {
        let window = WindowIdentity {
            handle: 1,
            process_id: 2,
        };
        let frame = |id| {
            CapturedFrame::from_bgra8(
                FrameId::new(id),
                TopologyGeneration::new(1),
                window,
                QpcTimestamp::new(id),
                100,
                100,
                96,
                96,
                400,
                vec![0; 100 * 100 * 4],
            )
            .expect("fixture frame is valid")
        };
        let response = |frame: &CapturedFrame, text: &str, x: f32| OcrResponse {
            request_id: OcrRequestId::new(),
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            model: OcrModel::PpOcrV6Medium,
            elapsed_ms: 1,
            items: vec![OcrItem {
                raw_text: text.to_owned(),
                confidence: 0.99,
                polygon: vec![
                    PolygonPoint { x, y: 70.0 },
                    PolygonPoint {
                        x: x + 20.0,
                        y: 70.0,
                    },
                    PolygonPoint {
                        x: x + 20.0,
                        y: 84.0,
                    },
                    PolygonPoint { x, y: 84.0 },
                ],
            }],
        };
        let mut builder = VisualSceneBuilder::new();
        let first_frame = frame(1);
        let first = builder
            .build(
                window,
                &first_frame,
                &[response(&first_frame, "旧消息", 10.0)],
                &SceneBuildOptions::default(),
            )
            .expect("first scene is valid");
        let second_frame = frame(2);
        let second = builder
            .build(
                window,
                &second_frame,
                &[response(&second_frame, "新消息", 10.0)],
                &SceneBuildOptions::default(),
            )
            .expect("second scene is valid");
        (
            Arc::try_unwrap(first).expect("fixture scene is unique"),
            Arc::try_unwrap(second).expect("fixture scene is unique"),
        )
    }

    #[test]
    fn missing_previous_scene_is_uncertain_for_new_text() {
        let (first, second) = scenes();
        let outcome = evaluate_visual_condition(
            Some(&second),
            None,
            &VisualCondition::NewTextExistsSince {
                query: VisualQuery {
                    text: "新消息".to_owned(),
                    exact: true,
                    region: None,
                },
                since_scene_id: first.scene_id,
                region: None,
            },
        );
        assert!(matches!(outcome, VerificationOutcome::Uncertain { .. }));
    }

    #[test]
    fn new_text_requires_a_changed_stable_identity() {
        let (first, second) = scenes();
        let outcome = evaluate_visual_condition(
            Some(&second),
            Some(&first),
            &VisualCondition::NewTextExistsSince {
                query: VisualQuery {
                    text: "新消息".to_owned(),
                    exact: true,
                    region: None,
                },
                since_scene_id: first.scene_id,
                region: None,
            },
        );
        assert!(outcome.is_confirmed());
        assert_eq!(
            second.viewport,
            PhysicalRect::new(0, 0, 100, 100).expect("fixture rect")
        );
    }
}
