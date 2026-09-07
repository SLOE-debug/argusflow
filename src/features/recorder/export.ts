import type { RecordingTrace } from './model';

/** 导出完整时间线及证据引用；实际 PNG 随演示包目录一起提供给多模态 AI。 */
export const traceJson = (trace: RecordingTrace) => JSON.stringify(trace, null, 2);
/** 下载完全独立的 JSON；自动持久化文件仍保留在桌面录制目录。 */
export function downloadTrace(trace: RecordingTrace): void {
  const url = URL.createObjectURL(new Blob([traceJson(trace)], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = `${trace.recording_id}-timeline.json`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
