import type { KeyChord, KeyboardKey, KeyboardModifier } from '../workflow';
import type { CandidateBasis, InspectionFailure, KeyboardDecodeFailure, RecordedSelector,
  RecordingDiagnostic, ResolutionBackend } from './inspectionContracts';
import type { RecordedOperation } from './model';

/** 稳定后端名称，用于解释定位来源。 */
export const BACKEND_LABELS = {
  managed_cdp: '浏览器 DOM', uia: 'Windows UIA', vision: '视觉 OCR', coordinate: '坐标',
} satisfies Record<ResolutionBackend, string>;
/** Selector 分值依据。 */
export const BASIS_LABELS = {
  automation_id: 'AutomationId', test_id: 'data-testid', stable_id: '稳定 ID',
  role_name: '角色和名称', stable_ancestor: '稳定祖先', class: '类名',
  dynamic_attribute: '动态属性', visual_text: '视觉文字', coordinate: '屏幕坐标',
} satisfies Record<CandidateBasis, string>;
const FAILURE_LABELS = {
  unmanaged_window: '没有匹配的已连接浏览器会话', context_changed: '窗口或焦点已改变',
  invalid_geometry: '无法确认坐标转换', no_element: '没有有效元素', unavailable: '能力不可用',
  timeout: '定位超时', unsupported_scope: '暂不支持该文档范围',
} satisfies Record<InspectionFailure, string>;
const DIAGNOSTIC_LABELS = {
  late_inspection: '解析开始过晚，已保留降级结果', input_gap: '输入有缺口，已分开步骤',
  unsupported_input: '包含暂不支持的输入', unpaired_mouse: '鼠标按下与释放未配对',
  redacted: '输入已遮盖', selector_uniqueness_unverified: '定位候选尚未验证唯一性',
} satisfies Record<Exclude<RecordingDiagnostic['type'], 'fallback' | 'keyboard_decode'>, string>;
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
export function operationLabel(operation: RecordedOperation): string {
  switch (operation.type) {
    case 'click': return ({ left: '点击', right: '右键点击', middle: '中键点击' } as const)[operation.button];
    case 'type_text': return operation.text.type === 'redacted' ? '输入已遮盖' : `输入「${operation.text.value}」`;
    case 'press_key': return `按键 ${chordLabel(operation.chord)}`;
  }
}
/** 将已有 KeyChord 契约展示为常用按键名。 */
export function chordLabel(chord: KeyChord): string {
  return [...chord.modifiers.map((modifier) => MODIFIER_LABELS[modifier]),
    chord.key.type === 'character' ? chord.key.value.toUpperCase() : KEY_LABELS[chord.key.type]].join('+');
}
/** 读取已生成选择器，不重新推断目标。 */
export function selectorLabel(selector: RecordedSelector): string {
  return selector.type === 'aql' ? selector.value.source : `(${selector.value.x}, ${selector.value.y})`;
}
/** 本地化后端失败链，不暴露 provider 原始错误。 */
export function diagnosticLabel(diagnostic: RecordingDiagnostic): string {
  switch (diagnostic.type) {
    case 'fallback': return `${BACKEND_LABELS[diagnostic.backend]}：${FAILURE_LABELS[diagnostic.reason]}`;
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
