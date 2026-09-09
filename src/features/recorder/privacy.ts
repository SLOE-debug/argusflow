import { invoke } from '@tauri-apps/api/core';
import type { CompletedRecording, RawTraceEvent } from './model';

/** 完整截图来源；局部图由后端从处理后的完整图同步生成。 */
export type PrivacyImageKind = 'window' | 'target' | 'screen';
/** 原图像素坐标中的遮盖区域。 */
export type PrivacyRect = Readonly<{ x: number; y: number; width: number; height: number }>;
/** Paddle 返回的原图坐标文字框。 */
export type PrivacyOcrItem = Readonly<{ raw_text: string; confidence: number; polygon: readonly Readonly<{ x: number; y: number }>[] }>;
/** 一次持久化操作，不能提交文件路径。 */
export type PrivacyEdit =
  | Readonly<{ type: 'erase_event'; sequence: number }>
  | Readonly<{ type: 'mosaic'; sequence: number; kind: PrivacyImageKind; rect: PrivacyRect }>;
/** 搜索结果保留来源，文字命中和截图区域不会混用。 */
export type PrivacyResult = Readonly<{ id: string; sequence: number; text: string }> & (
  | Readonly<{ source: 'text' }>
  | Readonly<{ source: 'image'; kind: PrivacyImageKind; rect: PrivacyRect }>
);
/** 已保存截图使用本地 Paddle 服务，不重新捕获屏幕。 */
export const recognizePrivacyImage = (recordingId: string, sequence: number, kind: PrivacyImageKind) =>
  invoke<readonly PrivacyOcrItem[]>('recognize_recording_screenshot', { recordingId, sequence, kind });
/** 处理后返回磁盘上的最新记录，供所有导出与预览同步。 */
export const editRecordingPrivacy = (recordingId: string, edits: readonly PrivacyEdit[]) =>
  invoke<CompletedRecording>('edit_recording_privacy', { recordingId, edits });

/** 只收集内容字段，避免把序号、坐标和协议键当作隐私命中。 */
export function recordingText(event: RawTraceEvent): string {
  const strings: string[] = [];
  if (event.input.type === 'key' && event.input.text?.type === 'plain') strings.push(event.input.text.value);
  if (event.input.type === 'clipboard' && event.input.content.type === 'text') strings.push(event.input.content.value);
  const context = event.evidence?.context;
  if (context) strings.push(context.title, context.executable_path ?? '');
  const entity = event.evidence?.ui_snapshot?.entity;
  if (entity) strings.push(entity.semantics.name ?? '', entity.page_url ?? '', ...entity.ancestors.map((ancestor) => ancestor.name ?? ''));
  return strings.join(' · ');
}

/** 连续按键的文字合并用于查找，命中时列出整段记录，包含释放键以免残留可逆键码。 */
export function recordingSearchText(events: readonly RawTraceEvent[]): ReadonlyMap<number, string> {
  const content = new Map(events.map((event) => [event.sequence, recordingText(event)]));
  let keys: RawTraceEvent[] = [];
  const flush = () => {
    const text = keys.map((event) => event.input.type === 'key' && event.input.phase === 'down' && event.input.text?.type === 'plain' ? event.input.text.value : '').join('');
    if (text) keys.forEach((event) => content.set(event.sequence, `${text} · ${content.get(event.sequence) ?? ''}`));
    keys = [];
  };
  for (const event of events) {
    if (event.input.type !== 'key') { flush(); continue; }
    const previous = keys.at(-1);
    if (previous && (event.elapsed_ms - previous.elapsed_ms > 2000 || event.evidence?.context?.window.handle !== previous.evidence?.context?.window.handle)) flush();
    keys.push(event);
  }
  flush();
  return content;
}

/** 多个关键词按任一命中匹配；留空列出全部可检查内容。 */
export const privacyKeywords = (query: string) => query.toLocaleLowerCase().split(/[\n,，]+/u).map((word) => word.trim()).filter(Boolean);
/** OCR 框外扩两像素，避免边缘笔画留在遮盖区外。 */
export function ocrPrivacyRect(item: PrivacyOcrItem, width: number, height: number): PrivacyRect | null {
  if (!item.polygon.length || item.polygon.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))) return null;
  const x = Math.max(0, Math.floor(Math.min(...item.polygon.map((point) => point.x))) - 2);
  const y = Math.max(0, Math.floor(Math.min(...item.polygon.map((point) => point.y))) - 2);
  const right = Math.min(width, Math.ceil(Math.max(...item.polygon.map((point) => point.x))) + 2);
  const bottom = Math.min(height, Math.ceil(Math.max(...item.polygon.map((point) => point.y))) + 2);
  return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
}
