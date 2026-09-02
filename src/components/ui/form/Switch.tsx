import type { ComponentPropsWithoutRef } from 'react';

/** 保留原生 checkbox 行为的开关属性。 */
export type SwitchProps = Readonly<
  Omit<ComponentPropsWithoutRef<'input'>, 'className' | 'type'>
>;

/** 用于即时开关设置的通用控件，同时保留键盘和表单语义。 */
export function Switch(props: SwitchProps) {
  return (
    <span className="relative inline-flex h-5 w-9 shrink-0">
      <input
        {...props}
        type="checkbox"
        className={
          'peer absolute inset-0 z-10 m-0 cursor-pointer appearance-none rounded-full ' +
          'outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-1 ' +
          'disabled:cursor-not-allowed disabled:opacity-50'
        }
      />
      <span
        className={
          'pointer-events-none absolute inset-0 rounded-full bg-slate-300 transition-colors ' +
          'peer-checked:bg-blue-600 peer-disabled:opacity-50'
        }
        aria-hidden="true"
      />
      <span
        className={
          'pointer-events-none absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow-sm ' +
          'transition-transform peer-checked:translate-x-4 peer-disabled:opacity-70'
        }
        aria-hidden="true"
      />
    </span>
  );
}
