/** Focus Mask 使用的图像像素点。 */
export type FocusPoint = Readonly<{ x: number; y: number }>;
/** polygon 缺失时使用的图像像素矩形。 */
export type FocusRect = Readonly<{ x: number; y: number; width: number; height: number }>;

/** 已由 Runtime 排序的合法查询候选；数组顺序就是 viewer 的 Candidate 顺序。 */
export type FocusCandidate = Readonly<{
  id: string;
  rawText: string;
  confidence: number;
  polygon: ReadonlyArray<FocusPoint>;
  bbox: FocusRect;
}>;

/** 严格复刻 Runtime 0/1/N 的 viewer 状态，不执行最高分自动选择。 */
export type FocusSelection =
  | Readonly<{ outcome: 'not_found'; candidates: readonly [] }>
  | Readonly<{ outcome: 'unique'; selected: FocusCandidate; candidates: readonly [FocusCandidate] }>
  | Readonly<{ outcome: 'ambiguous'; candidates: ReadonlyArray<FocusCandidate> }>;

/** 从合法候选集推导纯解释层状态；N 个候选时永远保持 ambiguous。 */
export function deriveFocusSelection(
  candidates: ReadonlyArray<FocusCandidate>,
): FocusSelection {
  if (candidates.length === 0) {
    return { outcome: 'not_found', candidates: [] };
  }
  if (candidates.length === 1) {
    return { outcome: 'unique', selected: candidates[0], candidates: [candidates[0]] };
  }
  return { outcome: 'ambiguous', candidates };
}
