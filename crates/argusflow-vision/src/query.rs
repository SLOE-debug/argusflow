//! VisualScene 上的 deterministic exact/fuzzy 查询。

mod aql;
mod diagnostics;

pub use aql::{
    VisionQueryCompileError, VisionQueryMetrics, VisionQueryPlan, VisionQueryResult,
    compile_vision_query, evaluate_vision_query, require_unique,
};
pub use diagnostics::{VisualQueryCandidateSummary, VisualQueryReport};

use argusflow_core::{AutomationError, VisualQuery};

use crate::{
    app_scene::{AppNodeRef, AppScene},
    ocr::normalize_text,
    region::normalized_region_to_physical,
    scene::{VisualNode, VisualScene},
};

/// 在一个进程的全部窗口上执行文本查询，保持 0/1/N 语义。
pub fn evaluate_app_query<'scene>(
    scene: &'scene AppScene,
    query: &VisualQuery,
) -> Result<AppNodeRef<'scene>, AutomationError> {
    let candidates = matching_app_nodes(scene, query);
    match candidates.as_slice() {
        [] => Err(AutomationError::TargetNotFound {
            query: query.text.clone(),
            details: format!("searched {} process windows", scene.windows.len()),
        }),
        [candidate] => Ok(*candidate),
        candidates => Err(AutomationError::AmbiguousTarget {
            query: query.text.clone(),
            matches: candidates.len(),
            details: "matching OCR text exists in multiple window nodes".to_owned(),
        }),
    }
}

/// 返回进程全部窗口中的匹配节点，窗口 Z-Order 优先于窗口内 reading order。
pub fn matching_app_nodes<'scene>(
    scene: &'scene AppScene,
    query: &VisualQuery,
) -> Vec<AppNodeRef<'scene>> {
    scene
        .windows
        .iter()
        .flat_map(|window| {
            matching_nodes(&window.scene, query)
                .into_iter()
                .map(|node| AppNodeRef {
                    window: &window.window,
                    scene: &window.scene,
                    node,
                })
        })
        .collect()
}

/// 查询唯一命中时返回事实节点；多候选只作为显式候选集合返回。
#[derive(Debug, Clone, Copy)]
pub enum VisualMatch<'scene> {
    /// 唯一命中。
    Unique(&'scene VisualNode),
}

/// 在 current viewport scene 上执行 exact/contains 查询，严格实现 0/1/N 语义。
pub fn evaluate_visual_query<'scene>(
    scene: &'scene VisualScene,
    query: &VisualQuery,
) -> Result<VisualMatch<'scene>, AutomationError> {
    let candidates = matching_nodes(scene, query);
    let report = VisualQueryReport::from_matches(scene, query, &candidates);
    match candidates.as_slice() {
        [] => Err(AutomationError::TargetNotFound {
            query: query.text.clone(),
            details: format!("；{}", report.summary()),
        }),
        [node] => Ok(VisualMatch::Unique(node)),
        _ => Err(AutomationError::AmbiguousTarget {
            query: query.text.clone(),
            matches: candidates.len(),
            details: format!("；{}", report.summary()),
        }),
    }
}

/// 返回全部匹配节点，调用方必须显式决定是否接受多个结果。
pub fn matching_nodes<'scene>(
    scene: &'scene VisualScene,
    query: &VisualQuery,
) -> Vec<&'scene VisualNode> {
    let expected = normalize_text(&query.text);
    if expected.is_empty() {
        return Vec::new();
    }
    let region = query
        .region
        .map(|region| normalized_region_to_physical(region, scene.viewport));
    if query.region.is_some() && region.flatten().is_none() {
        return Vec::new();
    }
    let region = region.flatten();
    scene
        .nodes
        .iter()
        .filter(|node| {
            if !region.map_or(true, |bounds| node.bbox.intersects(bounds)) {
                return false;
            }
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

    use argusflow_core::{NormalizedRect, VisualQuery, WindowIdentity};

    use super::*;
    use crate::{
        frame::{FrameId, QpcTimestamp, TopologyGeneration},
        image::CapturedFrame,
        ocr::{
            OcrItem, OcrModel, OcrPreprocessingSummary, OcrRequestId, OcrResponse, PolygonPoint,
        },
        scene::{SceneBuildOptions, VisualSceneBuilder},
    };

    fn scene() -> VisualScene {
        let window = WindowIdentity {
            handle: 1,
            process_id: 2,
        };
        let frame = CapturedFrame::from_bgra8(
            FrameId::new(1),
            TopologyGeneration::new(0),
            window,
            QpcTimestamp::new(1),
            100,
            100,
            96,
            96,
            400,
            vec![0; 100 * 100 * 4],
        )
        .expect("fixture frame is valid");
        let response = OcrResponse {
            request_id: OcrRequestId::new(),
            frame_id: frame.frame_id,
            topology_generation: frame.topology_generation,
            model: OcrModel::PpOcrV6Small,
            elapsed_ms: 1,
            preprocessing: OcrPreprocessingSummary {
                input_width: 100,
                input_height: 100,
                output_width: 100,
                output_height: 100,
                contrast_enhanced: false,
                sharpened: false,
                binarized: false,
            },
            timings: crate::ocr::OcrTimingSummary {
                preprocess_elapsed_ms: 0,
                inference_elapsed_ms: 1,
            },
            model_input: None,
            items: vec![
                OcrItem {
                    raw_text: "确定".to_owned(),
                    confidence: 0.99,
                    polygon: square(10.0, 10.0),
                },
                OcrItem {
                    raw_text: "确定".to_owned(),
                    confidence: 0.98,
                    polygon: square(50.0, 10.0),
                },
            ],
        };
        Arc::try_unwrap(
            VisualSceneBuilder::new()
                .build(window, &frame, &[response], &SceneBuildOptions::default())
                .expect("scene builds"),
        )
        .expect("fixture has one owner")
    }

    fn square(x: f32, y: f32) -> Vec<PolygonPoint> {
        vec![
            PolygonPoint { x, y },
            PolygonPoint { x: x + 10.0, y },
            PolygonPoint {
                x: x + 10.0,
                y: y + 10.0,
            },
            PolygonPoint { x, y: y + 10.0 },
        ]
    }

    #[test]
    fn exact_query_rejects_multiple_matches() {
        let scene = scene();
        let error = evaluate_visual_query(
            &scene,
            &VisualQuery {
                text: "确定".to_owned(),
                exact: true,
                region: None,
            },
        )
        .expect_err("two exact nodes must be ambiguous");
        assert!(matches!(
            &error,
            AutomationError::AmbiguousTarget { matches: 2, details, .. }
                if details.contains("PP-OCRv6 Small")
                    && details.contains("“确定” [10,10,10×10] 99%")
                    && details.contains("“确定” [50,10,10×10] 98%")
        ));
    }

    #[test]
    fn missing_query_reports_the_last_ocr_result_summary() {
        let scene = scene();
        let error = evaluate_visual_query(
            &scene,
            &VisualQuery {
                text: "取消".to_owned(),
                exact: true,
                region: None,
            },
        )
        .expect_err("the missing query should keep OCR diagnostics");

        assert!(matches!(
            &error,
            AutomationError::TargetNotFound { details, .. }
                if details.contains("命中 0/2 段") && details.contains("耗时 1 ms")
        ));
    }

    #[test]
    fn normalized_region_limits_exact_candidates_before_selection() {
        let scene = scene();
        let query = VisualQuery {
            text: "确定".to_owned(),
            exact: true,
            region: Some(NormalizedRect::new(0.0, 0.0, 0.4, 1.0).expect("valid region")),
        };

        let VisualMatch::Unique(node) = evaluate_visual_query(&scene, &query)
            .expect("the region should keep exactly one candidate");
        assert_eq!(node.bbox.x, 10);
    }
}
