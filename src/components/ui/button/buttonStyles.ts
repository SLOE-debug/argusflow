import type { ButtonSize, ButtonVariant } from './types';

/** Button 基础视觉参数；业务层只覆盖布局，不复制交互状态样式。 */
export type ButtonStyleOptions = Readonly<{
  /** 按钮语义色。 */
  variant?: ButtonVariant;
  /** 按钮视觉密度。 */
  size?: ButtonSize;
  /** 业务层附加的布局样式。 */
  className?: string;
}>;

/** 按语义统一按钮的颜色、边框、焦点和禁用反馈。 */
export function buttonStyles({
  variant = 'secondary',
  size = 'standard',
  className = '',
}: ButtonStyleOptions = {}): string {
  const variantClassName = BUTTON_VARIANT_CLASS_NAMES[variant];
  const sizeClassName = BUTTON_SIZE_CLASS_NAMES[size];
  return `${BASE_BUTTON_CLASS_NAME} ${sizeClassName} ${variantClassName} ${className}`;
}

/** Button 的共用交互反馈。 */
const BASE_BUTTON_CLASS_NAME = [
  'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap',
  '[&>svg]:shrink-0',
  'rounded-md border text-[12px] leading-none outline-none transition-colors',
  'focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-1',
  'disabled:cursor-not-allowed disabled:opacity-45',
].join(' ');

/** Button 语义变体的 Tailwind 样式。 */
const BUTTON_VARIANT_CLASS_NAMES: Readonly<Record<ButtonVariant, string>> = {
  primary: 'border-blue-600 bg-blue-600 font-semibold text-white hover:border-blue-700 hover:bg-blue-700',
  secondary: 'border-slate-300 bg-white font-medium text-slate-700 hover:bg-slate-50',
  ghost: 'border-transparent bg-transparent font-medium text-slate-600 hover:bg-slate-100 hover:text-slate-900',
  danger: 'border-rose-200 bg-rose-50 font-semibold text-rose-700 hover:bg-rose-100',
};

/** Button 密度的高度、内边距和字号。 */
const BUTTON_SIZE_CLASS_NAMES: Readonly<Record<ButtonSize, string>> = {
  compact: 'h-[26px] px-2.5',
  standard: 'h-8 px-3',
};
