import type { DraftState, LanguageService } from './contracts';

/** 导出只使用当前有效草稿，不能回退到前一次有效源码。 */
export function exportSource(state: DraftState): string | undefined {
  return state.status === 'ready' && state.analysis.diagnostics.length === 0
    ? state.analysis.english : undefined;
}
/** 导入仅接受 UTF-8 英文 AQL 文件；中文编辑文本由语言服务转换。 */
export async function readAqlFile(file: File, service: LanguageService): Promise<string> {
  if (!file.name.toLowerCase().endsWith('.aql')) throw new Error('请选择 .aql 文件。');
  if (file.size > 65_536) throw new Error('文件超过 64 KiB，请缩短查询后重试。');
  const buffer = await file.arrayBuffer();
  let source: string;
  try { source = new TextDecoder('utf-8', { fatal: true }).decode(buffer); }
  catch { throw new Error('文件不是 UTF-8 编码，请转换编码后重试。'); }
  return service.importEnglish(source);
}
/** 下载规范英文文本，Object URL 在浏览器接收下载后释放。 */
export function downloadAql(source: string): void {
  const url = URL.createObjectURL(new Blob([source], { type: 'text/plain;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'query.aql';
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
