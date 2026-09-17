import type { RecordingRecord } from "./model";

const MAX_RECORDS = 4096;
const MAX_CHARACTERS = 8 * 1024 * 1024;
/** 时间线仅缓存有界窗口；完整日志留在磁盘，可从头重新分页。 */
export function appendWindow(
  previous: readonly RecordingRecord[],
  incoming: readonly RecordingRecord[],
): readonly RecordingRecord[] {
  const result = [...previous, ...incoming].slice(-MAX_RECORDS);
  let characters = 0;
  for (let index = result.length - 1; index >= 0; index--) {
    characters += JSON.stringify(result[index]).length;
    if (characters > MAX_CHARACTERS) return result.slice(index + 1);
  }
  return result;
}
