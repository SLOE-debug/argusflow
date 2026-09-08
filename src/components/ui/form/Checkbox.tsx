import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from 'react';
import Check from 'lucide-react/dist/esm/icons/check.mjs';

/** 原生 checkbox 中除固定 type 外的全部标准属性。 */
export type CheckboxProps = Omit<ComponentPropsWithoutRef<'input'>, 'type'>;

/** 与工作流表单统一尺寸、焦点和禁用状态的基础复选框。 */
export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(function Checkbox({
  className = '',
  ...props
}, ref) {
  return (
    <span className="relative inline-flex size-4 shrink-0 align-middle">
    <input
      {...props}
      ref={ref}
      type="checkbox"
      className={`peer size-4 cursor-pointer appearance-none rounded border border-slate-300 bg-white shadow-sm transition-colors checked:border-blue-600 checked:bg-blue-600 hover:border-blue-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500 disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
    />
    <Check
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 size-4 p-0.5 text-white opacity-0 peer-checked:opacity-100 peer-disabled:peer-checked:opacity-40"
      strokeWidth={3}
    />
    </span>
  );
});
