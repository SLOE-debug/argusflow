import type { ValueExpr } from './contracts';

/** 相对于当前应用窗口视觉视口的归一化识别区域。 */
export type NormalizedRect = Readonly<{
  /** 左侧起点，范围为 `[0, 1]`。 */
  x: number;
  /** 顶部起点，范围为 `[0, 1]`。 */
  y: number;
  /** 归一化宽度，必须大于零。 */
  width: number;
  /** 归一化高度，必须大于零。 */
  height: number;
}>;

/** Studio 持久化的视觉查询表达式。 */
export type VisualQueryExpr = Readonly<{
  /** 运行时解析为目标文字的值表达式。 */
  text: ValueExpr;
  /** 是否要求识别文字完全相等。 */
  exact: boolean;
  /** 可选的归一化识别区域；使用 null 保持工作流 JSON 不含 undefined。 */
  region: NormalizedRect | null;
}>;
