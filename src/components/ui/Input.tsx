import type { ComponentPropsWithoutRef, ReactNode } from 'react';

import {
  FORM_CONTROL_DENSITY_CLASS_NAMES,
  type FormControlDensity,
} from './formControlStyles';

export type InputProps = Readonly<
  Omit<ComponentPropsWithoutRef<'input'>, 'className' | 'size'> & {
    /** 控件视觉密度；紧凑模式适用于桌面标题栏。 */
    density?: FormControlDensity;
    /** 原生输入框的附加样式。 */
    className?: string;
    /** 输入框外层容器的附加布局与尺寸。 */
    containerClassName?: string;
    /** 输入内容之前的图标或其他纯视觉元素。 */
    startAdornment?: ReactNode;
  }
>;

/** 统一边框、聚焦状态和视觉密度的基础文本输入框。 */
export function Input({
  density = 'standard',
  className = '',
  containerClassName = '',
  startAdornment,
  ...inputProps
}: InputProps) {
  /** 当前密度对应的公共容器和文字样式。 */
  const densityClassNames = FORM_CONTROL_DENSITY_CLASS_NAMES[density];
  const contentSpacingClassName = startAdornment ? 'pl-1.5' : '';

  return (
    <div
      className={
        'flex items-center rounded-md border border-slate-200 bg-slate-50 ' +
        'text-slate-400 focus-within:border-blue-400 focus-within:bg-white ' +
        `${densityClassNames.container} ${containerClassName}`
      }
    >
      {startAdornment}
      <input
        {...inputProps}
        className={
          'min-w-0 flex-1 border-0 bg-transparent text-slate-700 outline-none ' +
          'placeholder:text-slate-400 disabled:cursor-not-allowed disabled:opacity-50 ' +
          `${densityClassNames.text} ${contentSpacingClassName} ${className}`
        }
      />
    </div>
  );
}

