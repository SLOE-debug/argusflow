import { ChevronDown } from 'lucide-react';
import {
  type ChangeEvent,
  type ComponentPropsWithoutRef,
  type ReactNode,
} from 'react';

import {
  FORM_CONTROL_DENSITY_CLASS_NAMES,
  type FormControlDensity,
} from './formControlStyles';

export type SelectOption<Value extends string> = Readonly<{
  /** 提交给调用方的稳定选项值。 */
  value: Value;
  /** 向用户展示的选项文字。 */
  label: string;
  /** 当前选项是否不可选择。 */
  disabled?: boolean;
}>;

export type SelectProps<Value extends string> = Readonly<
  Omit<
    ComponentPropsWithoutRef<'select'>,
    'children' | 'className' | 'defaultValue' | 'multiple' | 'onChange' | 'size' | 'value'
  > & {
    /** 当前受控选项值。 */
    value: Value;
    /** 只读选项清单。 */
    options: ReadonlyArray<SelectOption<Value>>;
    /** 选项变化时返回强类型值。 */
    onValueChange?: (value: Value) => void;
    /** 控件视觉密度；紧凑模式适用于桌面标题栏。 */
    density?: FormControlDensity;
    /** 原生选择框的附加样式。 */
    className?: string;
    /** 选择框外层容器的附加布局与尺寸。 */
    containerClassName?: string;
    /** 选项文字之前的图标或其他纯视觉元素。 */
    startAdornment?: ReactNode;
  }
>;

/** 使用原生选择语义和统一视觉密度的基础 Select 控件。 */
export function Select<Value extends string>({
  value,
  options,
  onValueChange,
  density = 'standard',
  className = '',
  containerClassName = '',
  startAdornment,
  ...selectProps
}: SelectProps<Value>) {
  /** 当前密度对应的公共容器和文字样式。 */
  const densityClassNames = FORM_CONTROL_DENSITY_CLASS_NAMES[density];
  const contentSpacingClassName = startAdornment ? 'pl-1.5' : '';
  const handleChange = (event: ChangeEvent<HTMLSelectElement>) => {
    /** 使用原始选项恢复强类型值，避免把任意 DOM 字符串直接断言为领域值。 */
    const selectedOption = options.find((option) => option.value === event.target.value);
    if (selectedOption) onValueChange?.(selectedOption.value);
  };

  return (
    <div
      className={
        'flex items-center rounded-md border border-slate-200 bg-slate-50 ' +
        'text-slate-600 focus-within:border-blue-400 focus-within:bg-white ' +
        `${densityClassNames.container} ${containerClassName}`
      }
    >
      {startAdornment}
      <select
        {...selectProps}
        value={value}
        onChange={handleChange}
        className={
          'min-w-0 flex-1 appearance-none border-0 bg-transparent text-slate-800 ' +
          'outline-none disabled:cursor-not-allowed disabled:opacity-50 ' +
          `${densityClassNames.text} ${contentSpacingClassName} ${className}`
        }
      >
        {options.map((option) => (
          <option
            key={option.value}
            value={option.value}
            disabled={option.disabled}
          >
            {option.label}
          </option>
        ))}
      </select>
      <ChevronDown
        className="pointer-events-none ml-1.5 size-2.5 shrink-0 text-slate-500"
        aria-hidden="true"
      />
    </div>
  );
}
