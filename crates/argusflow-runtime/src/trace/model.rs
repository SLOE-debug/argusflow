use std::collections::BTreeMap;

use argusflow_core::{ExecutionEvent, WorkflowDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Run Trace 持久化协议版本。
pub const RUN_TRACE_SCHEMA_VERSION: u32 = 1;

/// Scene 投影协议版本；v1 的 ASCII 投影不会被新执行台读取。
pub const RUN_SCENE_PROJECTION_SCHEMA_VERSION: u16 = 2;

/// 启动运行时冻结的最小展示信息，不改变可执行工作流 schema。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPresentationSnapshot {
    /// 展示快照协议版本。
    pub schema_version: u16,
    /// 工作流节点 ID 到用户可见名称的只读映射。
    pub node_labels: BTreeMap<String, String>,
}

impl RunPresentationSnapshot {
    /// 创建当前版本的展示快照。
    pub fn new(node_labels: BTreeMap<String, String>) -> Self {
        Self {
            schema_version: 1,
            node_labels,
        }
    }
}

impl Default for RunPresentationSnapshot {
    fn default() -> Self {
        Self::new(BTreeMap::new())
    }
}

/// 单次运行保存诊断事实的详细程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTraceLevel {
    /// 不创建持久化运行记录。
    Off,
    /// 保存工作流、输入来源和生命周期事件。
    Basic,
    /// 额外允许视觉与 OCR artifact。
    Diagnostics,
    /// 保存完整场景、刷新与候选过滤事实。
    Forensics,
}

/// Run Manifest 的稳定生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// 目录已经建立，执行任务尚未开始。
    Starting,
    /// 工作流正在运行。
    Running,
    /// 工作流成功结束。
    Completed,
    /// 工作流以业务错误结束。
    Failed,
    /// 进程退出前未完成的遗留运行。
    Crashed,
}

/// Run List 首屏可直接读取、无需扫描事件文件的索引记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    /// 持久化协议版本。
    pub schema_version: u32,
    /// 与 Runtime 和全部 ExecutionEvent 完全一致的运行 ID。
    pub run_id: Uuid,
    /// 执行时工作流快照的稳定 ID。
    pub workflow_id: Uuid,
    /// 执行时工作流名称。
    pub workflow_name: String,
    /// Unix epoch 毫秒时间，供前端按本地时区展示。
    pub started_at_unix_ms: u64,
    /// 运行结束时间；未结束时为空。
    pub finished_at_unix_ms: Option<u64>,
    /// 当前生命周期状态。
    pub status: RunStatus,
    /// 本次记录的诊断级别。
    pub trace_level: RunTraceLevel,
    /// 已成功追加到 JSONL 的事件数量。
    pub event_count: u64,
    /// Trace 持久化是否发生过不影响业务执行的失败。
    pub trace_degraded: bool,
    /// 失败节点；工作流级失败时为空。
    pub failed_node_id: Option<String>,
    /// 原始业务错误摘要，不被 Trace I/O 错误覆盖。
    pub failure_message: Option<String>,
}

/// JSONL 中的诊断 envelope；产品 ExecutionEvent 原样保留在 `event` 字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunTraceEvent {
    /// 持久化协议版本。
    pub schema_version: u32,
    /// 当前只有 Runtime 事件，因此与 workflow sequence 同步；后续子系统事件共用该序列。
    pub trace_sequence: u64,
    /// 写入时的 Unix epoch 毫秒。
    pub timestamp_unix_ms: u64,
    /// 原始产品事件，保持 Live Studio 与历史视图的事实一致。
    pub event: ExecutionEvent,
}

/// 一个值输入在运行时的稳定来源类别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedInputSource {
    /// 工作流定义中的字面量。
    Literal,
    /// 本次运行提供的瞬时输入；值一律脱敏，避免当前 schema 无敏感标记时泄漏。
    WorkflowInput { key: String },
    /// 运行内变量。
    Variable { name: String },
    /// 已成功节点的公开输出。
    Node { node_id: String },
    /// 受限表达式计算结果。
    Expression { expression: String },
    /// 不包含 OS handle 的逻辑资源引用。
    Resource {
        producer_node_id: String,
        output_name: String,
    },
}

/// 节点开始前已经解析完成的单个输入事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedInputField {
    /// 节点声明顺序中的稳定展示名称。
    pub name: String,
    /// PreparedNode 声明的开放值类型；资源输入使用资源类型 ID。
    pub expected_type: String,
    /// 输入来自何处，而不是从格式化字符串反向推断。
    pub source: ResolvedInputSource,
    /// 最终 JSON 值；敏感输入或资源引用为空。
    pub value: Option<Value>,
    /// 是否因隐私约束省略最终值。
    pub redacted: bool,
}

/// 节点目录中的 `resolved-inputs.json` 契约。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNodeInputs {
    /// 持久化协议版本。
    pub schema_version: u32,
    /// 扁平执行计划中的节点 ID。
    pub node_id: String,
    /// 节点执行序号，避免循环或未来重试覆盖既有现场。
    pub node_sequence: u64,
    /// 已解析值输入与逻辑资源输入。
    pub fields: Vec<ResolvedInputField>,
}

/// 节点成功后保存的公开输出名称，不复制业务值或资源 handle。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunNodeOutputs {
    /// 持久化协议版本。
    pub schema_version: u32,
    /// 可被后续 ValueExpr 引用的公开值名称。
    pub output_names: Vec<String>,
    /// 本次节点产生的逻辑资源端口名称。
    pub resource_names: Vec<String>,
}

/// Run Inspector 按节点读取的输入与输出摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunNodeTrace {
    /// 扁平执行节点 ID。
    pub node_id: String,
    /// 与节点目录一致的执行序号。
    pub node_sequence: u64,
    /// 节点开始前的最终解析输入。
    pub resolved_inputs: ResolvedNodeInputs,
    /// 节点失败时为空。
    pub outputs: Option<RunNodeOutputs>,
}

/// Run API 返回的 Manifest 与执行时工作流快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunDetails {
    /// 运行索引信息。
    pub manifest: RunManifest,
    /// 执行时的原始工作流定义，不读取当前编辑器状态。
    pub workflow: WorkflowDefinition,
    /// 运行开始时冻结的用户可见节点名称。
    pub presentation: RunPresentationSnapshot,
    /// 按执行序号排列的节点诊断摘要。
    pub nodes: Vec<RunNodeTrace>,
    /// 只暴露 artifact_id 的安全媒体索引，不返回磁盘路径。
    pub artifacts: Vec<RunArtifactSummary>,
    /// Vision crate 写入的结构化查询/候选/选择事实。
    pub query_traces: Vec<RunVisualQueryTrace>,
}

/// 前端可直接绘制的物理像素矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// OCR polygon 中的帧内浮点坐标。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RunPixelPoint {
    pub x: f32,
    pub y: f32,
}

/// 一次进程 Scene 中的窗口投影。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSceneWindowProjection {
    /// HWND 以字符串保存，避免 JavaScript 丢失 64 位整数精度。
    pub window_handle: String,
    pub scene_id: u64,
    pub frame_id: u64,
    pub z_order: usize,
    pub foreground: bool,
    /// 窗口在虚拟屏幕中的真实物理像素边界。
    pub screen_bounds: RunPixelRect,
    /// 捕获帧本地坐标范围。
    pub frame_bounds: RunPixelRect,
}

/// 一个 OCR 文本节点的精确帧坐标、屏幕坐标与质量事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSceneNodeProjection {
    pub node_id: String,
    pub scene_id: u64,
    pub frame_id: u64,
    pub window_handle: String,
    pub text: String,
    pub frame_bbox: RunPixelRect,
    pub screen_bbox: RunPixelRect,
    pub polygon: Vec<RunPixelPoint>,
    pub confidence: f32,
    pub source: String,
}

/// 结构化候选身份，窗口和 Scene 共同消除节点 ID 的跨窗口碰撞。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSceneNodeRef {
    pub window_handle: String,
    pub scene_id: u64,
    pub node_id: String,
}

/// 一次查询看到的真实坐标 Scene 投影。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSceneProjection {
    pub schema_version: u16,
    pub windows: Vec<RunSceneWindowProjection>,
    pub nodes: Vec<RunSceneNodeProjection>,
}

/// 查询选择的稳定结果类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunVisualSelectionOutcome {
    NotFound,
    Unique,
    Multiple,
    Ambiguous,
    RejectedConfidence,
}

/// AQL 查询性能计数器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunVisualQueryMetrics {
    pub elapsed_us: u64,
    pub exact_index_hits: usize,
    pub scanned_nodes: usize,
    pub spatial_candidates: usize,
}

/// 运行执行台读取的一次强类型视觉查询事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunVisualQueryTrace {
    pub schema_version: u16,
    pub run_id: Uuid,
    pub node_id: String,
    /// 节点执行序号用于区分循环中的同一节点。
    pub node_sequence: u64,
    pub query: String,
    pub outcome: RunVisualSelectionOutcome,
    pub candidate_nodes: Vec<RunSceneNodeRef>,
    pub selected_node: Option<RunSceneNodeRef>,
    pub metrics: RunVisualQueryMetrics,
    pub projection: RunSceneProjection,
}

/// Run Inspector 可读取的一张诊断图片。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunArtifactSummary {
    /// 只能由 Store 自己解析的稳定引用。
    pub artifact_id: String,
    /// 捕获帧、OCR ROI 或 exact model input。
    pub kind: RunArtifactKind,
    /// 图片 MIME 类型。
    pub mime_type: String,
    /// 可选原始像素宽度。
    pub width: Option<u32>,
    /// 可选原始像素高度。
    pub height: Option<u32>,
    /// OCR artifact 的 request ID；捕获帧为空。
    pub request_id: Option<Uuid>,
    /// 触发捕获或 OCR 的节点执行序号。
    pub node_sequence: u64,
    /// 捕获来源 HWND 的十进制字符串。
    pub window_handle: String,
    /// 关联捕获帧 ID。
    pub frame_id: u64,
}

/// 诊断图片的稳定语义类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunArtifactKind {
    /// WGC 合成后的完整帧。
    CapturedFrame,
    /// Worker 前的 Rust OCR ROI。
    OcrSourceRoi,
    /// 真正传给 Paddle predict 的最终像素。
    OcrModelInput,
}
