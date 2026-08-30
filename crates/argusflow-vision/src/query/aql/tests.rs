use std::sync::Arc;

use argusflow_core::{AutomationError, WindowIdentity};
use argusflow_query::parse_query;

use super::*;
use crate::{
    AppWindowScene, CapturedFrame, FrameId, OcrItem, OcrModel, OcrPreprocessingSummary,
    OcrRequestId, OcrResponse, OcrTimingSummary, PolygonPoint, QpcTimestamp, SceneBuildOptions,
    TopologyGeneration, VisualSceneBuilder, WindowDescriptor,
};

#[test]
fn exact_text_uses_scene_index_and_preserves_unique_semantics() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[("锚点", 10, 10), ("其它", 30, 10)],
    )]);
    let query = parse_query("text(name = \"锚点\")").expect("AQL should parse");
    let plan = compile_vision_query(&query).expect("text matcher should compile");
    let result =
        evaluate_vision_query(&app, &plan, "text(name = \"锚点\")").expect("query should execute");
    let (selected, metrics) =
        require_unique(&result, "text(name = \"锚点\")").expect("exact text should be unique");

    assert_eq!(selected.node.raw_text, "锚点");
    assert_eq!(metrics.exact_index_hits, 1);
    assert_eq!(metrics.scanned_nodes, 1);
}

#[test]
fn nearest_only_ranks_targets_from_the_anchor_window() {
    let app = app_scene(vec![
        window_scene(
            1,
            7,
            &[("锚点", 10, 10), ("目标", 10, 40), ("目标", 10, 80)],
        ),
        window_scene(2, 7, &[("目标", 10, 20)]),
    ]);
    let source = "nearest(anchor = text(name = \"锚点\"), target = text(name = \"目标\"), direction = below, index = 2)";
    let query = parse_query(source).expect("nearest AQL should parse");
    let plan = compile_vision_query(&query).expect("nearest should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("nearest should execute");
    let (selected, metrics) = require_unique(&result, source).expect("second rank is unique");

    assert_eq!(selected.window.identity.handle, 1);
    assert_eq!(selected.node.bbox.y, 80);
    assert_eq!(metrics.spatial_candidates, 2);
}

#[test]
fn nearest_rejects_exact_ties_at_the_selected_rank() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[("锚点", 50, 10), ("目标", 40, 40), ("目标", 60, 40)],
    )]);
    let source = "nearest(anchor = text(name = \"锚点\"), target = text(name = \"目标\"), direction = below, index = 1)";
    let query = parse_query(source).expect("nearest AQL should parse");
    let plan = compile_vision_query(&query).expect("nearest should compile");
    let error = evaluate_vision_query(&app, &plan, source)
        .expect_err("equal geometry ranks must be ambiguous");

    assert!(matches!(
        error,
        AutomationError::AmbiguousTarget { matches: 2, .. }
    ));
}

#[test]
fn most_used_anchor_selects_the_exact_contact_title_below_its_header() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[
            ("最常使用", 43, 22),
            ("崽崽", 98, 79),
            ("群聊", 42, 141),
            ("包含:崽崽", 98, 215),
            ("包含:崽崽", 98, 300),
            ("搜索网络结果", 50, 650),
        ],
    )]);
    let source = "nearest(anchor = text(name = \"最常使用\"), target = text(name = \"崽崽\"), direction = below, index = 1)";
    let query = parse_query(source).expect("contact result AQL should parse");
    let plan = compile_vision_query(&query).expect("contact result AQL should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("query should execute");
    let (selected, metrics) =
        require_unique(&result, source).expect("most-used contact title is unique");

    assert_eq!(selected.node.raw_text, "崽崽");
    assert_eq!(selected.node.bbox.y, 79);
    assert_eq!(metrics.spatial_candidates, 1);
}

#[test]
fn window_close_anchor_selects_conversation_header_among_duplicate_contact_text() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[
            ("X", 185, 5),
            ("崽崽", 20, 20),
            ("崽崽", 80, 21),
            ("崽崽", 25, 70),
        ],
    )]);
    let source = "nearest(anchor = text(name = \"X\"), target = text(name = \"崽崽\"), direction = left, index = 1)";
    let query = parse_query(source).expect("conversation header AQL should parse");
    let plan = compile_vision_query(&query).expect("conversation header AQL should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("query should execute");
    let (selected, metrics) =
        require_unique(&result, source).expect("conversation header should be unique");

    assert_eq!(selected.node.bbox.x, 80);
    assert_eq!(selected.node.bbox.y, 21);
    assert_eq!(metrics.spatial_candidates, 3);
}

fn app_scene(windows: Vec<AppWindowScene>) -> AppScene {
    AppScene {
        process_id: 7,
        windows,
    }
}

fn window_scene(handle: u64, process_id: u32, items: &[(&str, i32, i32)]) -> AppWindowScene {
    let identity = WindowIdentity { handle, process_id };
    let frame = CapturedFrame::from_bgra8(
        FrameId::new(handle),
        TopologyGeneration::new(1),
        identity,
        QpcTimestamp::new(handle),
        200,
        120,
        96,
        96,
        800,
        vec![0; 200 * 120 * 4],
    )
    .expect("fixture frame should be valid");
    let response = OcrResponse {
        request_id: OcrRequestId::new(),
        frame_id: frame.frame_id,
        topology_generation: frame.topology_generation,
        model: OcrModel::PpOcrV6Small,
        elapsed_ms: 1,
        preprocessing: OcrPreprocessingSummary {
            input_width: 200,
            input_height: 120,
            output_width: 200,
            output_height: 120,
            contrast_enhanced: false,
            sharpened: false,
            binarized: false,
        },
        timings: OcrTimingSummary {
            preprocess_elapsed_ms: 0,
            inference_elapsed_ms: 1,
        },
        model_input: None,
        items: items
            .iter()
            .map(|(text, x, y)| OcrItem {
                raw_text: (*text).to_owned(),
                confidence: 0.99,
                polygon: polygon(*x as f32, *y as f32),
            })
            .collect(),
    };
    let scene = VisualSceneBuilder::new()
        .build(identity, &frame, &[response], &SceneBuildOptions::default())
        .expect("fixture scene should build");
    AppWindowScene {
        window: WindowDescriptor {
            identity,
            owner_handle: None,
            z_order: handle as usize,
            screen_bounds: PhysicalRect::new(0, 0, 200, 120).expect("valid bounds"),
            foreground: handle == 1,
        },
        scene: Arc::clone(&scene),
    }
}

fn polygon(x: f32, y: f32) -> Vec<PolygonPoint> {
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
