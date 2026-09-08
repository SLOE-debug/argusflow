//! 用户演示事件时间线：事实与证据，不包含工作流操作或执行定位器。

use crate::{RawInput, ScreenshotEvidence};
use argusflow_core::{InspectedEntity, InspectionContext, InspectionFailure};
use serde::{Deserialize, Serialize};

/// 结构化证据来源；图像不伪装成元素。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceBackend {
    /// 已托管的浏览器页面快照。
    ManagedCdp,
    /// 原生可访问性元素快照。
    Uia,
}

/// 不含 provider 原文的证据缺失诊断。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordingDiagnostic {
    /// 无法从物理按键可靠恢复提交文字。
    KeyboardDecode {
        /// 有限失败分类。
        reason: crate::KeyboardDecodeFailure,
    },
    /// 结构化检查失败，仍保留像素与原始事件。
    InspectionFailed {
        /// 实际尝试的来源。
        backend: EvidenceBackend,
        /// 无敏感原文的原因。
        reason: InspectionFailure,
    },
    /// 事件窗口无法确认。
    ContextUnavailable {
        /// 平台失败分类。
        reason: InspectionFailure,
    },
    /// 截图未成功冻结或保存，禁止在停止后补拍历史画面。
    ScreenshotUnavailable {
        /// 平台或存储失败分类。
        reason: InspectionFailure,
    },
    /// 检查开始太晚，未采集当前元素冒充历史事实。
    LateInspection,
    /// 有界采集链丢失了事件。
    InputGap,
    /// 敏感或未知字段的可逆键盘内容已遮盖。
    Redacted,
}

/// 一个事件的独立证据，可只含窗口与截图。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventEvidence {
    /// 点击通知时保存的目标证据，不能由随后同坐标的另一个窗口替代。
    pub click_target: Option<ScreenshotEvidence>,
    /// 事件发生时冻结的应用及窗口身份。
    pub context: Option<InspectionContext>,
    /// UIA/CDP 观察值及采样时间，不产生 selector 或执行目标。
    pub ui_snapshot: Option<UiSnapshot>,
    /// 独立的操作后主要图像，保留实际采样时间与稳定状态。
    pub screenshot: Option<ScreenshotEvidence>,
    /// 完整保留缺失原因，失败不使事件消失。
    pub diagnostics: Vec<RecordingDiagnostic>,
}

/// 结构化快照来源、内容与时间不可分离，禁止来源与元素不一致。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSnapshot {
    /// 实际返回观察结果的后端。
    pub backend: EvidenceBackend,
    /// 只读观察事实，不是执行定位器。
    pub entity: InspectedEntity,
    /// 查询开始相对录制起点的毫秒数。
    pub observed_at_ms: u64,
    /// 查询耗时，表明异步观察与原始事件之间的时序差。
    pub observation_duration_ms: u64,
}

/// 按真实输入保存的单个事件，不合并成 Click/TypeText。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTraceEvent {
    /// 捕获序号；丢弃事件表现为缺口，同一时间用此序号排序。
    pub sequence: u64,
    /// Win32 原始毫秒时钟，约 49 天回绕。
    pub timestamp_ms: u32,
    /// 展开回绕后的录制相对时间，单位毫秒。
    pub elapsed_ms: u64,
    /// 鼠标、键盘、窗口或剪贴板的观察事实。
    pub input: RawInput,
    /// 高频移动可无证据，其他事件尽可能采集。
    pub evidence: Option<EventEvidence>,
    /// 输入解码、遮盖与丢弃诊断。
    pub diagnostics: Vec<RecordingDiagnostic>,
}

impl RawTraceEvent {
    /// 移动段回指完整原始序号范围，单事件仍只占一个源序号。
    pub fn last_source_sequence(&self) -> u64 {
        match &self.input {
            RawInput::PointerMotion(motion) => motion.end_sequence,
            _ => self.sequence,
        }
    }

    /// 移动段使用真实结束时间，不能把录制时长缩短到该段起点。
    pub fn ended_ms(&self) -> u64 {
        match &self.input {
            RawInput::PointerMotion(motion) => motion.ended_ms,
            _ => self.elapsed_ms,
        }
    }
}

/// AI 与后续 compiler 共用的事件事实来源。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventTimeline {
    /// 按 (elapsed_ms, sequence) 排序；连续移动以有来源范围的轨迹表达。
    pub events: Vec<RawTraceEvent>,
}

impl EventTimeline {
    /// 将连续鼠标采样合并为轨迹；点击、输入、窗口、剪贴板、证据和缺口不跨越。
    pub fn compact_pointer_motion(&mut self) {
        crate::motion_compaction::compact(self);
    }
}

/// 完整多模态演示包：timeline JSON 与引用的 PNG 必须一起传递。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingTrace {
    /// 当前事件协议版本；不兼容旧 Semantic Trace。
    pub schema_version: u16,
    /// 同时作为本地演示包目录 UUID。
    pub recording_id: uuid::Uuid,
    /// 事件绝对时间原点，Unix 毫秒。
    pub started_at_unix_ms: u64,
    /// 唯一事件源，不派生预编译操作层。
    pub timeline: EventTimeline,
    /// 有界队列与容量造成的事件缺失数量。
    pub dropped_events: u64,
}
