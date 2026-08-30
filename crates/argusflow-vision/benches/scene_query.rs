//! 2k OCR 节点硬预算基准；只在显式 `cargo bench` 时执行。

use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_query::parse_query;
use argusflow_vision::{
    AppScene, AppWindowScene, CapturedFrame, FrameId, OcrItem, OcrModel, OcrPreprocessingSummary,
    OcrRequestId, OcrResponse, OcrTimingSummary, PhysicalRect, PolygonPoint, QpcTimestamp,
    SceneBuildOptions, SceneId, TopologyGeneration, VisualNode, VisualNodeSource,
    VisualSceneBuilder, VisualSceneIndex, WindowDescriptor, compile_vision_query,
    evaluate_vision_query,
};

const NODE_COUNT: usize = 2_000;

fn main() {
    let nodes = synthetic_nodes();
    let index_p95 = measure(200, || {
        black_box(VisualSceneIndex::build(black_box(&nodes)));
    });
    let scene = synthetic_scene();
    let exact_source = "text(name = \"锚点\")";
    let exact = compile_vision_query(&parse_query(exact_source).expect("exact query parses"))
        .expect("exact query compiles");
    let nearest_source = "nearest(anchor = text(name = \"锚点\"), target = text(name = \"目标\"), direction = below, index = 1)";
    let nearest = compile_vision_query(&parse_query(nearest_source).expect("nearest query parses"))
        .expect("nearest query compiles");
    let exact_p95 = measure(1_000, || {
        black_box(
            evaluate_vision_query(&scene, &exact, exact_source).expect("exact query executes"),
        );
    });
    let nearest_p95 = measure(300, || {
        black_box(
            evaluate_vision_query(&scene, &nearest, nearest_source)
                .expect("nearest query executes"),
        );
    });

    println!(
        "2k nodes p95: index={:?}, exact={:?}, nearest={:?}",
        index_p95, exact_p95, nearest_p95,
    );
    assert!(
        index_p95 <= Duration::from_millis(5),
        "index p95 exceeded 5ms"
    );
    assert!(
        exact_p95 <= Duration::from_micros(500),
        "exact p95 exceeded 0.5ms"
    );
    assert!(
        nearest_p95 <= Duration::from_millis(2),
        "nearest p95 exceeded 2ms"
    );
}

fn measure(iterations: usize, mut operation: impl FnMut()) -> Duration {
    for _ in 0..20 {
        operation();
    }
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    samples[(samples.len() * 95 / 100).min(samples.len() - 1)]
}

fn synthetic_nodes() -> Vec<VisualNode> {
    (0..NODE_COUNT)
        .filter_map(|index| {
            let text = if index == 0 { "锚点" } else { "目标" };
            let x = (index % 50 * 30) as f32;
            let y = (index / 50 * 20) as f32;
            VisualNode::from_ocr(
                SceneId::new(1),
                text.to_owned(),
                0.99,
                polygon(x, y),
                VisualNodeSource::OcrSmall,
            )
        })
        .collect()
}

fn synthetic_scene() -> AppScene {
    let identity = WindowIdentity {
        handle: 1,
        process_id: 7,
    };
    let frame = CapturedFrame::from_bgra8(
        FrameId::new(1),
        TopologyGeneration::new(1),
        identity,
        QpcTimestamp::new(1),
        1_600,
        900,
        96,
        96,
        6_400,
        vec![0; 1_600 * 900 * 4],
    )
    .expect("synthetic frame is valid");
    let response = OcrResponse {
        request_id: OcrRequestId::new(),
        frame_id: frame.frame_id,
        topology_generation: frame.topology_generation,
        model: OcrModel::PpOcrV6Small,
        elapsed_ms: 1,
        preprocessing: OcrPreprocessingSummary {
            input_width: 1_600,
            input_height: 900,
            output_width: 1_600,
            output_height: 900,
            contrast_enhanced: false,
            sharpened: false,
            binarized: false,
        },
        timings: OcrTimingSummary {
            preprocess_elapsed_ms: 0,
            inference_elapsed_ms: 1,
        },
        model_input: None,
        items: synthetic_nodes()
            .into_iter()
            .map(|node| OcrItem {
                raw_text: node.raw_text,
                confidence: node.confidence,
                polygon: node.polygon,
            })
            .collect(),
    };
    let scene = VisualSceneBuilder::new()
        .build(identity, &frame, &[response], &SceneBuildOptions::default())
        .expect("synthetic scene builds");
    AppScene {
        process_id: 7,
        windows: vec![AppWindowScene {
            window: WindowDescriptor {
                identity,
                owner_handle: None,
                z_order: 0,
                screen_bounds: PhysicalRect::new(0, 0, 1_600, 900).expect("valid bounds"),
                foreground: true,
            },
            scene: Arc::clone(&scene),
        }],
    }
}

fn polygon(x: f32, y: f32) -> Vec<PolygonPoint> {
    vec![
        PolygonPoint { x, y },
        PolygonPoint { x: x + 20.0, y },
        PolygonPoint {
            x: x + 20.0,
            y: y + 12.0,
        },
        PolygonPoint { x, y: y + 12.0 },
    ]
}
