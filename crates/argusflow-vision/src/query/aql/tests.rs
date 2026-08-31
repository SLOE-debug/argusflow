use std::sync::Arc;

use argusflow_core::{AutomationError, WindowIdentity};
use argusflow_query::parse_query;

use super::*;
use crate::{
    AppWindowScene, CapturedFrame, FrameId, OcrItem, OcrModel, OcrPreprocessingSummary,
    OcrRequestId, OcrResponse, OcrTimingSummary, PhysicalRect, PolygonPoint, QpcTimestamp,
    SceneBuildOptions, TopologyGeneration, VisualSceneBuilder, WindowDescriptor,
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
fn top_right_viewport_anchor_selects_conversation_header_among_duplicate_contact_text() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[("崽崽", 20, 20), ("崽崽", 170, 21), ("崽崽", 25, 70)],
    )]);
    let source = "nearest(anchor = viewport_corner(position = top_right), target = text(name = \"崽崽\"), direction = any, index = 1)";
    let query = parse_query(source).expect("conversation header AQL should parse");
    let plan = compile_vision_query(&query).expect("conversation header AQL should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("query should execute");
    let (selected, metrics) =
        require_unique(&result, source).expect("conversation header should be unique");

    assert_eq!(selected.node.bbox.x, 170);
    assert_eq!(selected.node.bbox.y, 21);
    assert_eq!(metrics.spatial_candidates, 3);
}

#[test]
fn search_anchor_requires_sidebar_contact_before_selecting_conversation_header() {
    let source = "nearest(anchor = text(name = \"搜索\"), target = text(name = \"崽崽\"), direction = any, index = 2)";
    let query = parse_query(source).expect("conversation header AQL should parse");
    let plan = compile_vision_query(&query).expect("conversation header AQL should compile");
    let pending = app_scene(vec![window_scene(
        1,
        7,
        &[
            ("搜索", 20, 20),
            ("文件传输助手", 100, 20),
            ("崽崽", 25, 80),
        ],
    )]);
    let opened = app_scene(vec![window_scene(
        1,
        7,
        &[("搜索", 20, 20), ("崽崽", 25, 80), ("崽崽", 170, 20)],
    )]);

    let pending_result =
        evaluate_vision_query(&pending, &plan, source).expect("pending query should execute");
    let opened_result =
        evaluate_vision_query(&opened, &plan, source).expect("opened query should execute");

    assert!(pending_result.matches.is_empty());
    assert_eq!(
        require_unique(&opened_result, source)
            .expect("opened conversation header should be unique")
            .0
            .node
            .bbox
            .x,
        170,
    );
}

#[test]
fn viewport_corner_ranking_survives_independent_axis_scaling() {
    let source = "nearest(anchor = viewport_corner(position = top_right), target = text(name = \"会话\"), direction = any, index = 1)";
    let query = parse_query(source).expect("viewport AQL should parse");
    let plan = compile_vision_query(&query).expect("viewport AQL should compile");
    let wide = app_scene(vec![window_scene_with_size(
        1,
        7,
        300,
        150,
        &[("会话", 20, 20), ("会话", 260, 20)],
    )]);
    let tall = app_scene(vec![window_scene_with_size(
        1,
        7,
        150,
        300,
        &[("会话", 10, 40), ("会话", 125, 40)],
    )]);

    let wide_result = evaluate_vision_query(&wide, &plan, source).expect("wide query executes");
    let tall_result = evaluate_vision_query(&tall, &plan, source).expect("tall query executes");

    assert_eq!(
        require_unique(&wide_result, source)
            .expect("wide is unique")
            .0
            .node
            .bbox
            .x,
        260
    );
    assert_eq!(
        require_unique(&tall_result, source)
            .expect("tall is unique")
            .0
            .node
            .bbox
            .x,
        125
    );
}

#[test]
fn viewport_edge_uses_real_candidate_ordinal() {
    let app = app_scene(vec![window_scene(
        1,
        7,
        &[("关键字", 10, 20), ("关键字", 40, 20), ("关键字", 90, 20)],
    )]);
    let source = "nearest(anchor = viewport_edge(side = left), target = text(name = \"关键字\"), direction = any, index = 2)";
    let query = parse_query(source).expect("edge AQL should parse");
    let plan = compile_vision_query(&query).expect("edge AQL should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("edge query executes");

    assert_eq!(
        require_unique(&result, source)
            .expect("second candidate is unique")
            .0
            .node
            .bbox
            .x,
        40
    );
}

#[test]
fn bottom_edge_selects_pending_input_over_repeated_message_bubbles() {
    let app = app_scene(vec![window_scene_with_size(
        1,
        7,
        200,
        200,
        &[("重复消息", 140, 50), ("重复消息", 20, 160)],
    )]);
    let source = "nearest(anchor = viewport_edge(side = bottom), target = text(name = \"重复消息\"), direction = any, index = 1)";
    let query = parse_query(source).expect("pending message AQL should parse");
    let plan = compile_vision_query(&query).expect("pending message AQL should compile");
    let result =
        evaluate_vision_query(&app, &plan, source).expect("pending message query executes");

    assert_eq!(
        require_unique(&result, source)
            .expect("bottom-most pending input is unique")
            .0
            .node
            .bbox
            .y,
        160
    );
}

#[test]
fn viewport_anchor_rejects_a_tie_containing_requested_rank() {
    let app = app_scene(vec![window_scene_with_size(
        1,
        7,
        200,
        200,
        &[("同距", 10, 20), ("同距", 20, 10)],
    )]);
    let source = "nearest(anchor = viewport_corner(position = top_left), target = text(name = \"同距\"), direction = any, index = 1)";
    let query = parse_query(source).expect("corner AQL should parse");
    let plan = compile_vision_query(&query).expect("corner AQL should compile");
    let error = evaluate_vision_query(&app, &plan, source).expect_err("tie must be ambiguous");

    assert!(matches!(
        error,
        AutomationError::AmbiguousTarget { matches: 2, .. }
    ));
}

#[test]
fn viewport_anchor_preserves_cross_window_ambiguity() {
    let app = app_scene(vec![
        window_scene(1, 7, &[("搜索", 10, 10)]),
        window_scene(2, 7, &[("搜索", 10, 10)]),
    ]);
    let source = "nearest(anchor = viewport_corner(position = top_left), target = text(name = \"搜索\"), direction = any, index = 1)";
    let query = parse_query(source).expect("corner AQL should parse");
    let plan = compile_vision_query(&query).expect("corner AQL should compile");
    let result = evaluate_vision_query(&app, &plan, source).expect("per-window ranking succeeds");
    let error = require_unique(&result, source).expect_err("two window matches stay ambiguous");

    assert!(matches!(
        error,
        AutomationError::AmbiguousTarget { matches: 2, .. }
    ));
}

fn app_scene(windows: Vec<AppWindowScene>) -> AppScene {
    AppScene {
        process_id: 7,
        windows,
    }
}

fn window_scene(handle: u64, process_id: u32, items: &[(&str, i32, i32)]) -> AppWindowScene {
    window_scene_with_size(handle, process_id, 200, 120, items)
}

/// 创建具有显式宽高的窗口 Scene，用于验证 X/Y 独立缩放。
fn window_scene_with_size(
    handle: u64,
    process_id: u32,
    width: u32,
    height: u32,
    items: &[(&str, i32, i32)],
) -> AppWindowScene {
    let identity = WindowIdentity { handle, process_id };
    let frame = CapturedFrame::from_bgra8(
        FrameId::new(handle),
        TopologyGeneration::new(1),
        identity,
        QpcTimestamp::new(handle),
        width,
        height,
        96,
        96,
        width as usize * 4,
        vec![0; (width * height * 4) as usize],
    )
    .expect("fixture frame should be valid");
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
            screen_bounds: PhysicalRect::new(0, 0, width, height).expect("valid bounds"),
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
