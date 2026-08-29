//! VisualSceneIndex 与 nearest 空间查询的分辨率无关回归测试。

use std::{collections::BTreeMap, sync::Arc};

use argusflow_core::WindowIdentity;
use argusflow_query::{parse_query, resolve_query_parameters};
use argusflow_vision::{
    CapturedFrame, FrameId, ObservationCoverage, ObservationState, OcrItem, OcrModel,
    OcrPreprocessingSummary, OcrRequestId, OcrResponse, PolygonPoint, QpcTimestamp,
    SceneBuildOptions, TopologyGeneration, VisionQueryExecutionError, VisualSceneBuilder,
    VisualSceneSnapshot, compile_vision_query, execute_unique_vision_query,
};

#[test]
fn nearest_selects_first_and_second_distance_rank() {
    let snapshot = fixture_snapshot(
        200,
        160,
        &[
            ("网络结果", 20.0, 20.0),
            ("目标群", 20.0, 60.0),
            ("目标群", 20.0, 110.0),
        ],
    );

    let first = execute(&snapshot, 1).expect("first distance rank should be unique");
    let second = execute(&snapshot, 2).expect("second distance rank should be unique");

    assert_eq!(first.bbox.y, 60);
    assert_eq!(second.bbox.y, 110);
}

#[test]
fn nearest_rejects_equal_distance_ties() {
    let snapshot = fixture_snapshot(
        240,
        160,
        &[
            ("网络结果", 80.0, 20.0),
            ("目标群", 20.0, 70.0),
            ("目标群", 140.0, 70.0),
        ],
    );

    let error = execute(&snapshot, 1).expect_err("equal edge gaps must remain ambiguous");

    assert!(matches!(
        error,
        VisionQueryExecutionError::TargetAmbiguous { matches: 2 }
    ));
}

#[test]
fn nearest_result_is_stable_across_resolution_scaling() {
    let small = fixture_snapshot(
        200,
        160,
        &[
            ("网络结果", 20.0, 20.0),
            ("目标群", 20.0, 60.0),
            ("目标群", 20.0, 110.0),
        ],
    );
    let large = fixture_snapshot(
        400,
        320,
        &[
            ("网络结果", 40.0, 40.0),
            ("目标群", 40.0, 120.0),
            ("目标群", 40.0, 220.0),
        ],
    );

    assert_eq!(
        execute(&small, 1).unwrap().raw_text,
        execute(&large, 1).unwrap().raw_text
    );
    assert_eq!(
        execute(&small, 2).unwrap().bbox.y * 2,
        execute(&large, 2).unwrap().bbox.y
    );
}

/// 解析、绑定并执行一个显式 nearest rank。
fn execute<'scene>(
    snapshot: &'scene VisualSceneSnapshot,
    index: usize,
) -> Result<&'scene argusflow_vision::VisualNode, VisionQueryExecutionError> {
    let parsed = parse_query(&format!(
        "nearest(anchor = text(name contains \"网络结果\"), target = text(name = $group_name), direction = below, index = {index})"
    ))
    .expect("fixture AQL parses");
    let resolved = resolve_query_parameters(
        &parsed,
        &BTreeMap::from([("group_name".to_owned(), "目标群".to_owned())]),
    )
    .expect("fixture binding resolves");
    let plan = compile_vision_query(&resolved).expect("fixture compiles for Vision");
    execute_unique_vision_query(snapshot, &plan)
}

/// 构造无需真实微信或在线 OCR worker 的合成场景。
fn fixture_snapshot(width: u32, height: u32, items: &[(&str, f32, f32)]) -> VisualSceneSnapshot {
    let window = WindowIdentity {
        handle: 1,
        process_id: 2,
    };
    let frame = CapturedFrame::from_bgra8(
        FrameId::new(1),
        TopologyGeneration::new(1),
        window,
        QpcTimestamp::new(1),
        width,
        height,
        96,
        96,
        width as usize * 4,
        vec![0; width as usize * height as usize * 4],
    )
    .expect("fixture frame is valid");
    let response = OcrResponse {
        request_id: OcrRequestId::new(),
        frame_id: frame.frame_id,
        topology_generation: frame.topology_generation,
        model: OcrModel::PpOcrV6Small,
        elapsed_ms: 1,
        preprocessing: OcrPreprocessingSummary {
            input_width: width,
            input_height: height,
            output_width: width,
            output_height: height,
            contrast_enhanced: false,
            sharpened: false,
        },
        timings: argusflow_vision::OcrTimingSummary {
            preprocess_elapsed_ms: 0,
            inference_elapsed_ms: 1,
        },
        model_input: None,
        items: items
            .iter()
            .map(|(text, x, y)| OcrItem {
                raw_text: (*text).to_owned(),
                confidence: 0.99,
                polygon: rectangle(*x, *y, 40.0, 16.0),
            })
            .collect(),
    };
    let scene = VisualSceneBuilder::new()
        .build(window, &frame, &[response], &SceneBuildOptions::default())
        .expect("fixture scene builds");
    VisualSceneSnapshot::new(
        Arc::clone(&scene),
        ObservationState {
            coverage: ObservationCoverage::Complete,
            fresh_regions: Vec::new(),
            dirty_regions: Vec::new(),
        },
    )
}

/// 创建矩形 OCR polygon。
fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Vec<PolygonPoint> {
    vec![
        PolygonPoint { x, y },
        PolygonPoint { x: x + width, y },
        PolygonPoint {
            x: x + width,
            y: y + height,
        },
        PolygonPoint { x, y: y + height },
    ]
}
