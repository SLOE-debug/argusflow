//! Raw Trace 与 Normalized Semantic Trace 的分离持久化契约。

use crate::{MouseButton, RawInput, RecordedText, SelectorCandidate};
use argusflow_core::{InspectedEntity, InspectionContext, InspectionFailure, KeyChord};
use serde::{Deserialize, Serialize};

/// 解析链后端；与执行器的 BackendKind 不混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionBackend {
    /// 当前已附加的 browser 页面。
    ManagedCdp,
    /// UIA cache point/focus inspection。
    Uia,
    /// 现有窗口 OCR Scene。
    Vision,
    /// 只保留物理位置/窗口上下文。
    Coordinate,
}

/// 无敏感 provider 原文的降级与 normalization 诊断。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordingDiagnostic {
    /// 不能可靠恢复提交字符的具体原因，不含原始输入。
    KeyboardDecode {
        /// 输入法、布局、窗口或组合键的明确失败分类。
        reason: crate::KeyboardDecodeFailure,
    },
    /// 后端未能解析，链继续向下。
    Fallback {
        /// 此次尝试的后端。
        backend: ResolutionBackend,
        /// 不含 provider 原始字符串的失败分类。
        reason: InspectionFailure,
    },
    /// 事件未在有界时延内开始检查，当前 UI 不能证明历史目标。
    LateInspection,
    /// 输入队列丢失事件，禁止跨缺口合并步骤。
    InputGap,
    /// 第一阶段未覆盖的按键、IME、拖拽或滚轮。
    UnsupportedInput,
    /// 未闭合的鼠标 down/up，不猜测 Click。
    UnpairedMouse,
    /// 自动遮盖敏感或未知字段。
    Redacted,
    /// Selector 只有稳定性评分，尚未在 live tree 验证唯一性。
    SelectorUniquenessUnverified,
}

/// 每次输入解析产生的不可变目标快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTarget {
    /// 实际点击窗口；窗口已销毁时明确缺失。
    pub context: Option<InspectionContext>,
    /// 最终解析后端。
    pub backend: ResolutionBackend,
    /// Coordinate fallback 时为 None。
    pub entity: Option<InspectedEntity>,
    /// 从高到低的确定性候选。
    pub selector_candidates: Vec<SelectorCandidate>,
    /// 首选候选的索引，避免复制 selector 后产生不一致。
    pub preferred_selector: Option<usize>,
    /// 0..=1 的观察/定位置信度，不表示回放成功率。
    pub confidence: f32,
    /// 按尝试顺序保存失败原因。
    pub diagnostics: Vec<RecordingDiagnostic>,
}

/// Raw Trace 已脱敏事件；包含解析上下文，移动事件允许不解析目标。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTraceEvent {
    /// Hook 的真实序号。
    pub sequence: u64,
    /// Hook 原始 Win32 毫秒时钟（u32，约 49 天回绕）。
    pub timestamp_ms: u32,
    /// 展开 Win32 u32 wrap 后的录制相对时间，毫秒。
    pub elapsed_ms: u64,
    /// 已脱敏的原始输入。
    pub input: RawInput,
    /// Down/keyboard 的实际语义上下文。
    pub target: Option<ResolvedTarget>,
    /// 输入级诊断。
    pub diagnostics: Vec<RecordingDiagnostic>,
}

/// 可直接交给 AI 做步骤整理的语义操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordedOperation {
    /// 从同键 down/up 合成，保留原始鼠标按键。
    Click {
        /// 实际鼠标按键；现有工作流仅直接支持左键 Click。
        button: MouseButton,
    },
    /// 连续键入的增量文本；没有读取全字段值，因此不冒充 SetValue。
    TypeText {
        /// 完整输入片段或 redaction 标记。
        text: RecordedText,
    },
    /// 现有工作流键盘契约。
    PressKey {
        /// 可写入现有 WorkflowDefinition 的键盘契约。
        chord: KeyChord,
    },
}

/// 一条已语义化记录；原始事件编号回指单独保存的 Raw Trace。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticRecord {
    /// 规范化后的连续编号。
    pub sequence: u64,
    /// 首个事件相对时间。
    pub started_ms: u64,
    /// 最后一个事件相对时间。
    pub ended_ms: u64,
    /// 参与合并的 Raw Trace 事件序号，保留可审计来源。
    pub raw_event_ids: Vec<u64>,
    /// 与 raw_event_ids 同序的已脱敏原始输入，可独立发送语义层给 AI。
    pub original_input: Vec<RawInput>,
    /// 归一化动作。
    pub operation: RecordedOperation,
    /// 应用、窗口、实体、候选、backend、confidence 和降级证据。
    pub target: ResolvedTarget,
}

/// 分离保存的输入事实层。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RawTrace {
    /// 已脱敏事件，严格按序号排序。
    pub events: Vec<RawTraceEvent>,
}

/// 可单独发送给 AI 的规范化语义层。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NormalizedSemanticTrace {
    /// 语义操作按真实输入次序排列。
    pub records: Vec<SemanticRecord>,
    /// 不属于具体可回放步骤的诊断。
    pub diagnostics: Vec<RecordingDiagnostic>,
}

/// 单次录制导出入口；两个 Trace 在磁盘上分别持久化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingTrace {
    /// 此协议当前版本，独立于 AQL v3。
    pub schema_version: u16,
    /// 随机录制 ID，用于本地目录名称。
    pub recording_id: uuid::Uuid,
    /// 录制起始 Unix 毫秒。
    pub started_at_unix_ms: u64,
    /// 输入事实层。
    pub raw: RawTrace,
    /// AI 输入层。
    pub normalized: NormalizedSemanticTrace,
    /// Hook 和 worker 有界队列丢弃的事件数。
    pub dropped_events: u64,
}
