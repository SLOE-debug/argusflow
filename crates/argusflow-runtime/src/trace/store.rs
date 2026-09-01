use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use argusflow_core::{
    ExecutionEvent, ExecutionEventKind, FlowComponentDefinition, RunInputs, WorkflowDefinition,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    files::{read_json, unix_time_ms, write_bytes_atomic, write_json_atomic},
    index::{read_artifact_summaries, read_node_traces, read_query_traces},
    model::{
        RUN_TRACE_SCHEMA_VERSION, ResolvedInputField, ResolvedInputSource, ResolvedNodeInputs,
        RunDetails, RunManifest, RunNodeOutputs, RunPresentationSnapshot, RunStatus, RunTraceEvent,
        RunTraceLevel, RunVisualQueryTrace,
    },
    session_helpers::{node_directory, resolved_source},
};
use crate::{NodeOutcome, PreparedNode, RunContext};

/// Engine 创建和查询运行记录所依赖的最小持久化边界。
pub trait RunTraceStore: Send + Sync + 'static {
    /// 在异步执行任务启动前固化一次 Run 的定义、组件和输入快照。
    fn start_run(
        &self,
        run_id: Uuid,
        workflow: &WorkflowDefinition,
        expanded_workflow: &WorkflowDefinition,
        components: &[FlowComponentDefinition],
        inputs: &RunInputs,
        presentation: &RunPresentationSnapshot,
    ) -> Result<Arc<dyn RunTraceSession>, String>;
}

/// Engine 在单次运行期间追加事实的 best-effort 会话。
pub trait RunTraceSession: Send + Sync + 'static {
    /// 保存经过组件 source map 恢复后的产品执行事件。
    fn record_event(&self, event: &ExecutionEvent);

    /// 在 NodeStarted 前保存最终值输入和逻辑资源来源。
    fn record_resolved_inputs(
        &self,
        node_sequence: u64,
        node_id: &str,
        node: &dyn PreparedNode,
        context: &RunContext,
    );

    /// 节点成功发布后保存输出名称，不复制潜在敏感业务值。
    fn record_outputs(&self, node_sequence: u64, node_id: &str, outcome: &NodeOutcome);

    /// 原子更新最终状态；错误仅降级 Trace，不改变 Engine 返回结果。
    fn finish(&self, status: RunStatus, failed_node_id: Option<&str>, message: Option<&str>);
}

/// 以 `.argusflow/runs/<run-id>` 为根的本地文件 Run Trace Store。
#[derive(Debug)]
pub struct FileRunTraceStore {
    root: PathBuf,
    trace_level: RunTraceLevel,
}

impl FileRunTraceStore {
    /// 创建本地 Store；目录只在第一次运行写入时建立。
    pub fn new(root: impl Into<PathBuf>, trace_level: RunTraceLevel) -> Self {
        let store = Self {
            root: root.into(),
            trace_level,
        };
        store.recover_crashed_runs();
        store
    }

    /// 返回稳定排序的全部运行索引，损坏或未完成写入的单项不会阻塞其余历史。
    pub fn list_runs(&self) -> Result<Vec<RunManifest>, String> {
        let mut manifests = Vec::new();
        if !self.root.exists() {
            return Ok(manifests);
        }
        let entries = fs::read_dir(&self.root).map_err(|error| error.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path().join("manifest.json");
            if let Ok(manifest) = read_json::<RunManifest>(&path) {
                manifests.push(manifest);
            }
        }
        manifests.sort_by_key(|manifest| std::cmp::Reverse(manifest.started_at_unix_ms));
        Ok(manifests)
    }

    /// 按 run_id 读取 Manifest 和执行时工作流快照。
    pub fn get_run(&self, run_id: Uuid) -> Result<RunDetails, String> {
        let directory = self.run_directory(run_id)?;
        Ok(RunDetails {
            manifest: read_json(&directory.join("manifest.json"))?,
            workflow: read_json(&directory.join("workflow/definition.json"))?,
            presentation: read_json(&directory.join("workflow/presentation.json"))
                .unwrap_or_default(),
            nodes: read_node_traces(&directory.join("nodes")),
            artifacts: read_artifact_summaries(&directory),
            query_traces: read_query_traces(&directory.join("vision/queries")),
        })
    }

    /// 容错读取 JSONL；进程崩溃留下的半行或单条损坏记录会被忽略。
    pub fn read_events(&self, run_id: Uuid) -> Result<Vec<RunTraceEvent>, String> {
        let path = self.run_directory(run_id)?.join("events/events.jsonl");
        let file = File::open(path).map_err(|error| error.to_string())?;
        Ok(BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect())
    }

    /// 以强类型 ID 写入同一次 OCR 的三层图像与结构化结果，不接受调用方磁盘路径。
    #[allow(clippy::too_many_arguments)]
    pub fn persist_ocr_artifacts(
        &self,
        run_id: Uuid,
        window_handle: u64,
        frame_id: u64,
        request_id: Uuid,
        frame_bmp: &[u8],
        source_roi_bmp: &[u8],
        model_input_png: Option<&[u8]>,
        request_metadata: &Value,
        response_metadata: &Value,
    ) -> Result<(), String> {
        let run_directory = self.run_directory(run_id)?;
        let frame_directory = run_directory
            .join("vision/frames")
            .join(window_handle.to_string())
            .join(frame_id.to_string());
        fs::create_dir_all(&frame_directory).map_err(|error| error.to_string())?;
        write_bytes_atomic(&frame_directory.join("frame.bmp"), frame_bmp)?;

        let ocr_directory = run_directory
            .join("vision/ocr")
            .join(request_id.to_string());
        fs::create_dir_all(&ocr_directory).map_err(|error| error.to_string())?;
        write_json_atomic(&ocr_directory.join("01-request.json"), request_metadata)?;
        write_bytes_atomic(&ocr_directory.join("02-source-roi.bmp"), source_roi_bmp)?;
        if let Some(model_input_png) = model_input_png {
            write_bytes_atomic(&ocr_directory.join("03-model-input.png"), model_input_png)?;
        }
        write_json_atomic(&ocr_directory.join("04-result.json"), response_metadata)
    }

    /// 按 Store 生成的 artifact_id 返回原始图片 bytes，拒绝任意路径和未知类别。
    pub fn read_artifact(&self, run_id: Uuid, artifact_id: &str) -> Result<Vec<u8>, String> {
        let run_directory = self.run_directory(run_id)?;
        let parts = artifact_id.split(':').collect::<Vec<_>>();
        let path = match parts.as_slice() {
            ["frame", window_handle, frame_id] => {
                let window_handle = window_handle
                    .parse::<u64>()
                    .map_err(|_| "无效的窗口 artifact ID")?;
                let frame_id = frame_id
                    .parse::<u64>()
                    .map_err(|_| "无效的帧 artifact ID")?;
                run_directory
                    .join("vision/frames")
                    .join(window_handle.to_string())
                    .join(frame_id.to_string())
                    .join("frame.bmp")
            }
            ["ocr", request_id, "source_roi"] => {
                let request_id =
                    Uuid::parse_str(request_id).map_err(|_| "无效的 OCR artifact ID")?;
                run_directory
                    .join("vision/ocr")
                    .join(request_id.to_string())
                    .join("02-source-roi.bmp")
            }
            ["ocr", request_id, "model_input"] => {
                let request_id =
                    Uuid::parse_str(request_id).map_err(|_| "无效的 OCR artifact ID")?;
                run_directory
                    .join("vision/ocr")
                    .join(request_id.to_string())
                    .join("03-model-input.png")
            }
            _ => return Err("未知的 Run artifact ID".to_owned()),
        };
        fs::read(path).map_err(|error| error.to_string())
    }

    /// 保存输入层产生的结构化 0/1/N 查询事实；node_id 只作为净化后的目录名。
    pub fn persist_query_trace(
        &self,
        run_id: Uuid,
        node_id: &str,
        node_sequence: u64,
        trace: &RunVisualQueryTrace,
    ) -> Result<(), String> {
        let safe_node_id = node_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let directory = self
            .run_directory(run_id)?
            .join("vision/queries")
            .join(safe_node_id);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let scene_sequence = trace
            .projection
            .windows
            .iter()
            .map(|window| window.scene_id)
            .max()
            .unwrap_or(0);
        write_json_atomic(
            &directory.join(format!("{node_sequence:020}-{scene_sequence:020}.json")),
            trace,
        )
    }

    /// 仅允许 UUID 映射到 Store 自己的子目录，调用方不能注入磁盘路径。
    fn run_directory(&self, run_id: Uuid) -> Result<PathBuf, String> {
        let directory = self.root.join(run_id.to_string());
        if directory.is_dir() {
            Ok(directory)
        } else {
            Err(format!("运行记录不存在：{run_id}"))
        }
    }

    /// 将上次进程退出遗留的 starting/running Manifest 修复为可读的 crashed 状态。
    fn recover_crashed_runs(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("manifest.json");
            let Ok(mut manifest) = read_json::<RunManifest>(&path) else {
                continue;
            };
            if matches!(manifest.status, RunStatus::Starting | RunStatus::Running) {
                manifest.status = RunStatus::Crashed;
                manifest.finished_at_unix_ms = Some(unix_time_ms());
                manifest.trace_degraded = true;
                let _ = write_json_atomic(&path, &manifest);
            }
        }
    }
}

impl RunTraceStore for FileRunTraceStore {
    fn start_run(
        &self,
        run_id: Uuid,
        workflow: &WorkflowDefinition,
        expanded_workflow: &WorkflowDefinition,
        components: &[FlowComponentDefinition],
        inputs: &RunInputs,
        presentation: &RunPresentationSnapshot,
    ) -> Result<Arc<dyn RunTraceSession>, String> {
        if self.trace_level == RunTraceLevel::Off {
            return Ok(Arc::new(DisabledRunTraceSession));
        }
        let directory = self.root.join(run_id.to_string());
        fs::create_dir_all(directory.join("workflow")).map_err(|error| error.to_string())?;
        fs::create_dir_all(directory.join("events")).map_err(|error| error.to_string())?;
        fs::create_dir_all(directory.join("nodes")).map_err(|error| error.to_string())?;

        write_json_atomic(&directory.join("workflow/definition.json"), workflow)?;
        write_json_atomic(&directory.join("workflow/presentation.json"), presentation)?;
        write_json_atomic(
            &directory.join("workflow/expanded-definition.json"),
            expanded_workflow,
        )?;
        write_json_atomic(&directory.join("workflow/components.json"), components)?;
        // 当前输入 schema 没有敏感标志，因此全部瞬时值都脱敏，只保存字段存在性。
        let redacted_inputs = inputs
            .values
            .keys()
            .map(|key| (key.clone(), json!({ "redacted": true, "value": null })))
            .collect::<serde_json::Map<_, _>>();
        write_json_atomic(
            &directory.join("workflow/run-inputs.json"),
            &Value::Object(redacted_inputs),
        )?;

        let manifest = RunManifest {
            schema_version: RUN_TRACE_SCHEMA_VERSION,
            run_id,
            workflow_id: workflow.id,
            workflow_name: workflow.name.clone(),
            started_at_unix_ms: unix_time_ms(),
            finished_at_unix_ms: None,
            status: RunStatus::Starting,
            trace_level: self.trace_level,
            event_count: 0,
            trace_degraded: false,
            failed_node_id: None,
            failure_message: None,
        };
        write_json_atomic(&directory.join("manifest.json"), &manifest)?;
        let events = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("events/events.jsonl"))
            .map_err(|error| error.to_string())?;
        Ok(Arc::new(FileRunTraceSession {
            directory,
            state: Mutex::new(SessionState {
                manifest,
                events: BufWriter::new(events),
                next_trace_sequence: 0,
            }),
        }))
    }
}

#[derive(Debug)]
struct FileRunTraceSession {
    directory: PathBuf,
    state: Mutex<SessionState>,
}

#[derive(Debug)]
struct SessionState {
    manifest: RunManifest,
    events: BufWriter<File>,
    next_trace_sequence: u64,
}

impl FileRunTraceSession {
    /// 在持锁状态执行一次写入，失败时只标记 Manifest 降级并继续业务路径。
    fn best_effort(&self, operation: impl FnOnce(&mut SessionState) -> Result<(), String>) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if operation(&mut state).is_err() {
            state.manifest.trace_degraded = true;
        }
        if write_json_atomic(&self.directory.join("manifest.json"), &state.manifest).is_err() {
            state.manifest.trace_degraded = true;
        }
    }
}

impl RunTraceSession for FileRunTraceSession {
    fn record_event(&self, event: &ExecutionEvent) {
        self.best_effort(|state| {
            if event.kind == ExecutionEventKind::WorkflowStarted {
                state.manifest.status = RunStatus::Running;
            }
            let mut persisted_event = event.clone();
            // Log 节点允许展示任意业务文本；历史 Trace 无敏感 schema 时不复制其正文。
            if persisted_event.kind == ExecutionEventKind::Log {
                persisted_event.message = Some("[已脱敏的运行日志]".to_owned());
            }
            let trace_event = RunTraceEvent {
                schema_version: RUN_TRACE_SCHEMA_VERSION,
                trace_sequence: state.next_trace_sequence,
                timestamp_unix_ms: unix_time_ms(),
                event: persisted_event,
            };
            serde_json::to_writer(&mut state.events, &trace_event)
                .map_err(|error| error.to_string())?;
            state
                .events
                .write_all(b"\n")
                .map_err(|error| error.to_string())?;
            if matches!(
                event.kind,
                ExecutionEventKind::WorkflowStarted
                    | ExecutionEventKind::NodeFailed
                    | ExecutionEventKind::WorkflowCompleted
                    | ExecutionEventKind::WorkflowFailed
            ) {
                state.events.flush().map_err(|error| error.to_string())?;
            }
            state.next_trace_sequence += 1;
            state.manifest.event_count += 1;
            Ok(())
        });
    }

    fn record_resolved_inputs(
        &self,
        node_sequence: u64,
        node_id: &str,
        node: &dyn PreparedNode,
        context: &RunContext,
    ) {
        let directory = self.directory.clone();
        self.best_effort(|_state| {
            let mut fields = node
                .value_inputs()
                .into_iter()
                .enumerate()
                .map(|(index, input)| {
                    let source = resolved_source(input.expression);
                    // 表达式和节点输出可能间接派生自瞬时输入，当前 schema 无污点标记时保守脱敏。
                    let redacted = matches!(
                        source,
                        ResolvedInputSource::WorkflowInput { .. }
                            | ResolvedInputSource::Expression { .. }
                            | ResolvedInputSource::Node { .. }
                    );
                    let value = if redacted {
                        None
                    } else {
                        Some(
                            context
                                .resolve_value(input.expression)
                                .map_err(|error| error.to_string())?,
                        )
                    };
                    Ok(ResolvedInputField {
                        name: format!("value_{index:03}"),
                        expected_type: input.expected_type.as_str().to_owned(),
                        source,
                        value,
                        redacted,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            fields.extend(
                node.resource_inputs()
                    .into_iter()
                    .enumerate()
                    .map(|(index, input)| ResolvedInputField {
                        name: format!("resource_{index:03}"),
                        expected_type: input.expected_type.as_str().to_owned(),
                        source: ResolvedInputSource::Resource {
                            producer_node_id: input.reference.producer_node_id.clone(),
                            output_name: input.reference.output_name.clone(),
                        },
                        value: None,
                        redacted: false,
                    }),
            );
            let inputs = ResolvedNodeInputs {
                schema_version: RUN_TRACE_SCHEMA_VERSION,
                node_id: node_id.to_owned(),
                node_sequence,
                fields,
            };
            let node_directory = node_directory(&directory, node_sequence, node_id);
            fs::create_dir_all(&node_directory).map_err(|error| error.to_string())?;
            write_json_atomic(&node_directory.join("resolved-inputs.json"), &inputs)
        });
    }

    fn record_outputs(&self, node_sequence: u64, node_id: &str, outcome: &NodeOutcome) {
        let directory = self.directory.clone();
        self.best_effort(|_state| {
            let node_directory = node_directory(&directory, node_sequence, node_id);
            fs::create_dir_all(&node_directory).map_err(|error| error.to_string())?;
            let outputs = RunNodeOutputs {
                schema_version: RUN_TRACE_SCHEMA_VERSION,
                output_names: outcome.outputs.keys().cloned().collect(),
                resource_names: outcome.resources.clone(),
            };
            write_json_atomic(&node_directory.join("outputs.json"), &outputs)
        });
    }

    fn finish(&self, status: RunStatus, failed_node_id: Option<&str>, message: Option<&str>) {
        self.best_effort(|state| {
            state.manifest.status = status;
            state.manifest.finished_at_unix_ms = Some(unix_time_ms());
            if let Some(failed_node_id) = failed_node_id {
                state.manifest.failed_node_id = Some(failed_node_id.to_owned());
            }
            state.manifest.failure_message = message.map(str::to_owned);
            state.events.flush().map_err(|error| error.to_string())
        });
    }
}
struct DisabledRunTraceSession;
impl RunTraceSession for DisabledRunTraceSession {
    fn record_event(&self, _event: &ExecutionEvent) {}
    fn record_resolved_inputs(
        &self,
        _node_sequence: u64,
        _node_id: &str,
        _node: &dyn PreparedNode,
        _context: &RunContext,
    ) {
    }
    fn record_outputs(&self, _node_sequence: u64, _node_id: &str, _outcome: &NodeOutcome) {}
    fn finish(&self, _status: RunStatus, _failed_node_id: Option<&str>, _message: Option<&str>) {}
}
