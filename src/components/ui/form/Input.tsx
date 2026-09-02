import type { ComponentPropsWithoutRef, ReactNode } from 'react';

import {
  FORM_CONTROL_DENSITY_CLASS_NAMES,
  type FormControlDensity,
} from './formControlStyles';

export type InputProps = Readonly<
  Omit<ComponentPropsWithoutRef<'input'>, 'className' | 'size'> & {
    /** 控件视觉密度；紧凑模式适用于桌面标题栏。 */
    density?: FormControlDensity;
    /** 控件轮廓；目录搜索等结构化区域可使用直角外观。 */
    shape?: 'rounded' | 'square';
    /** 原生输入框的附加样式。 */
    className?: string;
    /** 输入框外层容器的附加布局与尺寸。 */
    containerClassName?: string;
    /** 输入内容之前的图标或其他纯视觉元素。 */
    startAdornment?: ReactNode;
    /** 输入内容之后的单位或其他纯视觉元素。 */
    endAdornment?: ReactNode;
  }
>;

/** 统一边框、聚焦状态和视觉密度的基础文本输入框。 */
export function Input({
  density = 'standard',
  shape = 'rounded',
  className = '',
  containerClassName = '',
  startAdornment,
  endAdornment,
  ...inputProps
}: InputProps) {
  /** 当前密度对应的公共容器和文字样式。 */
  const densityClassNames = FORM_CONTROL_DENSITY_CLASS_NAMES[density];
  /** 外形只改变轮廓，不改变输入框的密度和交互状态。 */
  const shapeClassName = shape === 'square' ? 'rounded-none' : 'rounded-md';
  const contentSpacingClassName = startAdornment ? 'pl-1.5' : '';

  return (
    <div
      className={
        `flex items-center ${shapeClassName} border border-slate-200 bg-slate-50 ` +
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
      {endAdornment}
    </div>
  );
}

