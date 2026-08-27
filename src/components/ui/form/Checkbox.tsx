import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from 'react';

/** 原生 checkbox 中除固定 type 外的全部标准属性。 */
export type CheckboxProps = Omit<ComponentPropsWithoutRef<'input'>, 'type'>;

/** 与工作流表单统一尺寸、焦点和禁用状态的基础复选框。 */
export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(function Checkbox({
  className = '',
  ...props
}, ref) {
  return (
    <input
      {...props}
      ref={ref}
      type="checkbox"
      className={`size-3.5 shrink-0 accent-blue-600 disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
    />
  );
});
