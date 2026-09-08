import type { KeyChord, KeyboardKey, KeyboardModifier } from '../workflow';
import type { InspectionFailure, KeyboardDecodeFailure,
  RecordingDiagnostic, EvidenceBackend } from './inspectionContracts';
import type { RawInput } from './model';
import { motionLabel } from './motion';

/** 稳定后端名称，用于解释定位来源。 */
export const BACKEND_LABELS = {
  managed_cdp: '浏览器 DOM', uia: 'Windows UIA',
} satisfies Record<EvidenceBackend, string>;
const FAILURE_LABELS = {
  unmanaged_window: '没有匹配的已连接浏览器会话', context_changed: '窗口或焦点已改变',
  invalid_geometry: '无法确认坐标转换', no_element: '没有有效元素', unavailable: '能力不可用',
  timeout: '定位超时', unsupported_scope: '暂不支持该文档范围',
} satisfies Record<InspectionFailure, string>;
const DIAGNOSTIC_LABELS = {
  late_inspection: '采样开始过晚，未补采当前界面', input_gap: '部分事件未能记录',
  redacted: '已按隐私设置遮盖内容',
} satisfies Record<'late_inspection' | 'input_gap' | 'redacted', string>;
/** 明确区分输入法限制、布局失败与窗口采样失败。 */
const KEYBOARD_FAILURE_LABELS = {
  invalid_key: '无法识别该按键', unsupported_chord: '暂不支持该组合键',
  missing_window: '未及时确认输入窗口，文字未保存', missing_thread: '无法确认键盘布局，文字未保存',
  input_method_active: '暂不支持输入法组合提交，组合期间的文字未录入',
  dead_key: '暂不支持死键组合，组合文字未录入', no_character: '键盘布局未产生可记录字符',
} satisfies Record<KeyboardDecodeFailure, string>;
const KEY_LABELS = {
  enter: 'Enter', escape: 'Esc', tab: 'Tab', backspace: 'Backspace', delete: 'Delete',
  arrow_left: '←', arrow_right: '→', arrow_up: '↑', arrow_down: '↓', home: 'Home',
  end: 'End', page_up: 'PageUp', page_down: 'PageDown',
} satisfies Record<Exclude<KeyboardKey['type'], 'character'>, string>;
const MODIFIER_LABELS = { control: 'Ctrl', alt: 'Alt', shift: 'Shift' } satisfies Record<KeyboardModifier, string>;

/** 不将 redaction 当作真实输入文字。 */
export function eventLabel(input: RawInput): string {
  switch (input.type) {
    case 'pointer_motion': return motionLabel(input);
    case 'mouse': return `${({ left: '左键', right: '右键', middle: '中键', x1: '侧键 1', x2: '侧键 2' } as const)[input.button]}${input.phase === 'down' ? '按下' : '释放'} (${input.point.x}, ${input.point.y})`;
    case 'move': return `鼠标移动 (${input.point.x}, ${input.point.y})`;
    case 'wheel': return `${input.horizontal ? '水平' : '垂直'}滚轮 ${input.delta}`;
    case 'window': return input.change === 'foreground' ? '切换窗口' : '窗口显示通知';
    case 'clipboard': return '剪贴板变化';
    case 'key': {
      const key = input.chord ? chordLabel(input.chord) : input.text?.type === 'plain' ? input.text.value
        : input.virtual_key !== null ? `VK ${input.virtual_key}` : '内容已遮盖';
      return `键盘${input.phase === 'down' ? '按下' : '释放'} · ${key}`;
    }
  }
}
/** 将已有 KeyChord 契约展示为常用按键名。 */
export function chordLabel(chord: KeyChord): string {
  return [...chord.modifiers.map((modifier) => MODIFIER_LABELS[modifier]),
    chord.key.type === 'character' ? chord.key.value.toUpperCase() : KEY_LABELS[chord.key.type]].join('+');
}
/** 本地化后端失败链，不暴露 provider 原始错误。 */
export function diagnosticLabel(diagnostic: RecordingDiagnostic): string {
  switch (diagnostic.type) {
    case 'inspection_failed': return `${BACKEND_LABELS[diagnostic.backend]}：${FAILURE_LABELS[diagnostic.reason]}`;
    case 'context_unavailable': return `窗口信息缺失：${FAILURE_LABELS[diagnostic.reason]}`;
    case 'screenshot_unavailable': return `截图未保存：${FAILURE_LABELS[diagnostic.reason]}`;
    case 'keyboard_decode': return KEYBOARD_FAILURE_LABELS[diagnostic.reason];
    default: return DIAGNOSTIC_LABELS[diagnostic.type];
  }
}
/** 从 Unix 毫秒展示本地录制时间。 */
export const recordingDate = (timestamp: number) => new Date(timestamp).toLocaleString('zh-CN', { hour12: false });
/** 相对时间按分秒展示，保持长录制计时。 */
export function durationLabel(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  return `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
}

/** 回看和选区使用毫秒精度，避免同一秒内的操作显示为同一个时间。 */
export function preciseDurationLabel(milliseconds: number): string {
  const value = Math.max(0, Math.floor(milliseconds));
  return `${durationLabel(value)}.${String(value % 1000).padStart(3, '0')}`;
}
