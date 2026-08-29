//! Run Inspector 首屏与按需详情读取所需的只读索引构造。

use std::{fs, path::Path};

use serde_json::Value;
use uuid::Uuid;

use super::{
    files::read_json,
    model::{
        ResolvedNodeInputs, RunArtifactKind, RunArtifactSummary, RunNodeOutputs, RunNodeTrace,
    },
};

pub(super) fn read_node_traces(root: &Path) -> Vec<RunNodeTrace> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut nodes = entries
        .flatten()
        .filter_map(|entry| {
            let inputs =
                read_json::<ResolvedNodeInputs>(&entry.path().join("resolved-inputs.json")).ok()?;
            let outputs = read_json::<RunNodeOutputs>(&entry.path().join("outputs.json")).ok();
            Some(RunNodeTrace {
                node_id: inputs.node_id.clone(),
                node_sequence: inputs.node_sequence,
                resolved_inputs: inputs,
                outputs,
            })
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.node_sequence);
    nodes
}

pub(super) fn read_artifact_summaries(run_directory: &Path) -> Vec<RunArtifactSummary> {
    let ocr_root = run_directory.join("vision/ocr");
    let Ok(entries) = fs::read_dir(ocr_root) else {
        return Vec::new();
    };
    let mut summaries = Vec::new();
    for entry in entries.flatten() {
        let Ok(request_id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let Ok(request) = read_json::<Value>(&entry.path().join("01-request.json")) else {
            continue;
        };
        append_source_artifacts(&mut summaries, request_id, &request);
        append_model_input_artifact(&mut summaries, request_id, &entry.path(), &request);
    }
    summaries.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    summaries.dedup_by(|left, right| left.artifact_id == right.artifact_id);
    summaries
}

fn append_source_artifacts(
    summaries: &mut Vec<RunArtifactSummary>,
    request_id: Uuid,
    request: &Value,
) {
    let frame_id = request.get("frame_id").and_then(Value::as_u64).unwrap_or(0);
    let dimension = |name: &str| {
        request
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    summaries.push(RunArtifactSummary {
        artifact_id: format!("frame:{frame_id}"),
        kind: RunArtifactKind::CapturedFrame,
        mime_type: "image/bmp".to_owned(),
        width: dimension("frame_width"),
        height: dimension("frame_height"),
        request_id: None,
        frame_id,
    });
    summaries.push(RunArtifactSummary {
        artifact_id: format!("ocr:{request_id}:source_roi"),
        kind: RunArtifactKind::OcrSourceRoi,
        mime_type: "image/bmp".to_owned(),
        width: dimension("source_width"),
        height: dimension("source_height"),
        request_id: Some(request_id),
        frame_id,
    });
}

fn append_model_input_artifact(
    summaries: &mut Vec<RunArtifactSummary>,
    request_id: Uuid,
    request_directory: &Path,
    request: &Value,
) {
    if !request_directory.join("03-model-input.png").is_file() {
        return;
    }
    let response = read_json::<Value>(&request_directory.join("04-result.json")).ok();
    let dimension = |pointer: &str| {
        response
            .as_ref()
            .and_then(|value| value.pointer(pointer))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    summaries.push(RunArtifactSummary {
        artifact_id: format!("ocr:{request_id}:model_input"),
        kind: RunArtifactKind::OcrModelInput,
        mime_type: "image/png".to_owned(),
        width: dimension("/preprocessing/output_width"),
        height: dimension("/preprocessing/output_height"),
        request_id: Some(request_id),
        frame_id: request.get("frame_id").and_then(Value::as_u64).unwrap_or(0),
    });
}

pub(super) fn read_query_traces(root: &Path) -> Vec<Value> {
    let Ok(node_entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut traces = node_entries
        .flatten()
        .flat_map(|node_entry| {
            fs::read_dir(node_entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| read_json::<Value>(&entry.path()).ok())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    traces.sort_by_key(|trace| trace.get("scene_id").and_then(Value::as_u64).unwrap_or(0));
    traces
}
