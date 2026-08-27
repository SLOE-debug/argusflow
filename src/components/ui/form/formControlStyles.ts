/** 通用表单控件支持的视觉密度。 */
export type FormControlDensity = 'compact' | 'standard';

/** Input 与 Select 共享的高度、内边距和字号，避免同密度控件产生视觉偏差。 */
export const FORM_CONTROL_DENSITY_CLASS_NAMES = {
  compact: {
    container: 'h-[26px] px-2',
    text: 'text-[12px] leading-none',
  },
  standard: {
    container: 'h-8 px-2.5',
    text: 'text-[12px] leading-4',
  },
} as const satisfies Readonly<
  Record<FormControlDensity, Readonly<{ container: string; text: string }>>
>;
