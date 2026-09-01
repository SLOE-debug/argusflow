use std::fs;

use serde_json::json;
use uuid::Uuid;

use super::{
    FileRunTraceStore, RunPixelRect, RunSceneProjection, RunSceneWindowProjection, RunTraceLevel,
    RunVisualQueryMetrics, RunVisualQueryTrace, RunVisualSelectionOutcome,
    index::read_query_traces,
};

#[test]
fn composite_window_and_frame_identity_prevents_artifact_overwrites() {
    let root = std::env::temp_dir().join(format!("argusflow-run-store-{}", Uuid::new_v4()));
    let run_id = Uuid::new_v4();
    fs::create_dir_all(root.join(run_id.to_string())).expect("fixture run directory should exist");
    let store = FileRunTraceStore::new(&root, RunTraceLevel::Diagnostics);

    for (window_handle, bytes) in [
        (101_u64, b"window-a".as_slice()),
        (202, b"window-b".as_slice()),
    ] {
        store
            .persist_ocr_artifacts(
                run_id,
                window_handle,
                7,
                Uuid::new_v4(),
                bytes,
                b"roi",
                None,
                &json!({
                    "window": { "handle": window_handle },
                    "frame_id": 7,
                    "node_sequence": 3,
                }),
                &json!({}),
            )
            .expect("artifact should persist");
    }

    assert_eq!(
        store
            .read_artifact(run_id, "frame:101:7")
            .expect("first frame should exist"),
        b"window-a",
    );
    assert_eq!(
        store
            .read_artifact(run_id, "frame:202:7")
            .expect("second frame should exist"),
        b"window-b",
    );
    fs::remove_dir_all(root).expect("fixture directory should be removable");
}

#[test]
fn typed_query_traces_sort_by_node_then_scene_sequence() {
    let root = std::env::temp_dir().join(format!("argusflow-query-store-{}", Uuid::new_v4()));
    let run_id = Uuid::new_v4();
    fs::create_dir_all(root.join(run_id.to_string())).expect("fixture run directory should exist");
    let store = FileRunTraceStore::new(&root, RunTraceLevel::Forensics);
    for trace in [
        query_trace(run_id, 8, 3),
        query_trace(run_id, 2, 9),
        query_trace(run_id, 2, 4),
    ] {
        store
            .persist_query_trace(run_id, "node", trace.node_sequence, &trace)
            .expect("query trace should persist");
    }

    let traces = read_query_traces(&root.join(run_id.to_string()).join("vision/queries"));
    let order = traces
        .iter()
        .map(|trace| (trace.node_sequence, trace.projection.windows[0].scene_id))
        .collect::<Vec<_>>();
    assert_eq!(order, vec![(2, 4), (2, 9), (8, 3)]);
    fs::remove_dir_all(root).expect("fixture directory should be removable");
}

fn query_trace(run_id: Uuid, node_sequence: u64, scene_id: u64) -> RunVisualQueryTrace {
    RunVisualQueryTrace {
        schema_version: 2,
        run_id,
        node_id: "node".to_owned(),
        node_sequence,
        query: "text(name=\"确定\")".to_owned(),
        outcome: RunVisualSelectionOutcome::NotFound,
        candidate_nodes: Vec::new(),
        selected_node: None,
        metrics: RunVisualQueryMetrics {
            elapsed_us: 1,
            exact_index_hits: 0,
            scanned_nodes: 0,
            spatial_candidates: 0,
        },
        projection: RunSceneProjection {
            schema_version: 2,
            windows: vec![RunSceneWindowProjection {
                window_handle: "101".to_owned(),
                scene_id,
                frame_id: 7,
                z_order: 0,
                foreground: true,
                screen_bounds: RunPixelRect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                frame_bounds: RunPixelRect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
            }],
            nodes: Vec::new(),
        },
    }
}
