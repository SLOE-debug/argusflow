import { invoke, isTauri } from '@tauri-apps/api/core';
import type { CompletedRecording, RecorderStatus, RecordingSummary } from './model';

/** 封闭命令注册，避免业务 UI 散落 IPC 字符串。 */
const COMMANDS = {
  start: 'start_recording', stop: 'stop_recording', status: 'get_recording_status',
  list: 'list_recordings', get: 'get_recording',
  screenshot: 'read_recording_screenshot',
} as const;
/** 浏览器预览可查看界面，但不能伪装拥有全局输入能力。 */
export const recorderAvailable = () => isTauri();
/** 显式安装真实全局 Hook。 */
export const startRecording = () => invoke<RecorderStatus>(COMMANDS.start);
/** 卸载 Hook、排空并保存。 */
export const stopRecording = () => invoke<CompletedRecording>(COMMANDS.stop);
/** 无输入内容的状态轮询。 */
export const getRecordingStatus = () => invoke<RecorderStatus>(COMMANDS.status);
/** 最新的完整录制摘要。 */
export const listRecordings = () => invoke<readonly RecordingSummary[]>(COMMANDS.list);
/** 只接受录制 ID，不接受任意读取路径。 */
export const getRecording = (recordingId: string) => (
  invoke<CompletedRecording>(COMMANDS.get, { recordingId })
);

/** 通过事件身份读取二进制 PNG，前端不能提交任意本地路径。 */
export const readRecordingScreenshot = (recordingId: string, sequence: number, kind: 'window' | 'crop') => (
  invoke<ArrayBuffer>(COMMANDS.screenshot, { recordingId, sequence, kind })
);
