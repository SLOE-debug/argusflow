import type { RecordingTrace } from './model';

/** 导出层的封闭集合，避免 UI 任意选择对象字段。 */
export type TraceLayer = 'normalized' | 'raw';
/** 只序列化后端已脱敏的对应层。 */
export const traceJson = (trace: RecordingTrace, layer: TraceLayer) => JSON.stringify(trace[layer], null, 2);
/** 下载完全独立的 JSON；自动持久化文件仍保留在桌面录制目录。 */
export function downloadTrace(trace: RecordingTrace, layer: TraceLayer): void {
  const url = URL.createObjectURL(new Blob([traceJson(trace, layer)], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = `${trace.recording_id}-${layer === 'raw' ? 'raw' : 'semantic'}.json`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
