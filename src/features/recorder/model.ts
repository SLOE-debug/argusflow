import type { KeyChord } from '../workflow';
import type { RecordingDiagnostic, EventEvidence, ScreenPoint } from './inspectionContracts';
import type { PointerMotion } from './motion';

/** 后端实际生命周期；保存失败不冒充仍在监听。 */
export type RecorderPhase = 'idle' | 'recording' | 'finishing' | 'awaiting_save';
/** 轮询仅传输计数，不传输输入内容。 */
export type RecorderStatus = Readonly<{
  /** 是否仍在监听或等待保存。 */
  phase: RecorderPhase;
  /** 本次会话 UUID，空闲时为空。 */
  recording_id: string | null;
  /** 计时起点，单位 Unix 毫秒。 */
  started_at_unix_ms: number | null;
  /** 已完成脱敏的原始事件数。 */
  processed_events: number;
  /** 有界队列和 Trace 容量造成的缺口数。 */
  dropped_events: number;
}>;
/** 敏感输入没有原文字段。 */
export type RecordedText = Readonly<{ type: 'plain'; value: string }> | Readonly<{ type: 'redacted' }>;
/** 第一阶段捕获的鼠标键。 */
export type MouseButton = 'left' | 'right' | 'middle' | 'x1' | 'x2';
/** 原始按下与释放。 */
export type InputPhase = 'down' | 'up';
/** 剪贴板变化时采集的内容或明确缺失状态。 */
export type ClipboardContent =
  | Readonly<{ type: 'text'; value: string; truncated: boolean }>
  | Readonly<{ type: 'empty' | 'non_text' | 'changed_before_capture' | 'unavailable' }>;
/** 已脱敏的原始输入，绝不接收未过滤的 Hook 数据。 */
export type RawInput =
  | Readonly<{ type: 'pointer_motion' } & PointerMotion>
  | Readonly<{ type: 'window'; window: Readonly<{ handle: number; process_id: number }>; change: 'foreground' | 'appeared' }>
  | Readonly<{ type: 'clipboard'; sequence_number: number; content: ClipboardContent }>
  | Readonly<{ type: 'mouse'; point: ScreenPoint; button: MouseButton; phase: InputPhase }>
  | Readonly<{ type: 'move'; point: ScreenPoint }>
  | Readonly<{ type: 'wheel'; point: ScreenPoint; delta: number; horizontal: boolean }>
  | Readonly<{ type: 'key'; virtual_key: number | null; scan_code: number | null;
    flags: number | null; phase: InputPhase; text: RecordedText | null; chord: KeyChord | null }>;
/** 原始事实层事件。 */
export type RawTraceEvent = Readonly<{
  /** Hook 的真实序号；轨迹段使用首个源事件序号。 */
  sequence: number;
  /** Win32 低 32 位事件时钟，单位毫秒。 */
  timestamp_ms: number;
  /** 展开时钟回绕后的录制相对毫秒数。 */
  elapsed_ms: number;
  /** Worker 已完成敏感处理的输入事实。 */
  input: RawInput;
  /** 只在需要反查的输入上存在。 */
  evidence: EventEvidence | null;
  /** 输入缺口、未支持操作和脱敏等事件级诊断。 */
  diagnostics: readonly RecordingDiagnostic[];
}>;
/** 单次完整录制。 */
export type RecordingTrace = Readonly<{
  /** 与 AQL v3 无关的录制协议版本。 */
  schema_version: 2;
  /** 本地存储目录使用的 UUID。 */
  recording_id: string;
  /** 录制开始时间，Unix 毫秒。 */
  started_at_unix_ms: number;
  /** 输入事实层，不包含未脱敏的临时 Hook 数据。 */
  timeline: Readonly<{ events: readonly RawTraceEvent[] }>;
  /** 整次录制丢弃事件总数。 */
  dropped_events: number;
}>;
/** 停止返回的保存文件和完整 Trace。 */
export type CompletedRecording = Readonly<{
  /** 后端自动保存的绝对路径，前端只显示，不接收任意路径读取。 */
  files: Readonly<{ timeline: string; evidence_directory: string; manifest: string }>;
  /** 事件时间线及证据引用。 */
  trace: RecordingTrace;
}>;
/** 历史列表不包含用户输入。 */
export type RecordingSummary = Readonly<{
  /** Manifest 中的录制协议版本。 */
  schema_version: number;
  /** 只用于按 UUID 加载录制。 */
  recording_id: string;
  /** 开始的 Unix 毫秒时间。 */
  started_at_unix_ms: number;
  /** 最后一个原始事件的相对毫秒数。 */
  duration_ms: number;
  /** 精简后时间线条目数，移动段计为一条。 */
  event_count: number;
  /** 附带图像证据的事件数。 */
  screenshot_count: number;
  /** 丢弃输入计数。 */
  dropped_events: number;
}>;
/** UI 初始状态没有推定已获知后端状态，ready 由 Hook 单独管理。 */
export const IDLE_RECORDER_STATUS: RecorderStatus = {
  phase: 'idle', recording_id: null, started_at_unix_ms: null,
  processed_events: 0, dropped_events: 0,
};
