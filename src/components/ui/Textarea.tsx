import type { ComponentPropsWithoutRef } from 'react';

export type TextareaProps = Readonly<
  Omit<ComponentPropsWithoutRef<'textarea'>, 'className'> & {
    /** 原生 textarea 的附加 Tailwind 样式。 */
    className?: string;
  }
>;

/** 统一边框、禁用态和聚焦反馈的多行文本基础控件。 */
export function Textarea({ className = '', ...textareaProps }: TextareaProps) {
  return (
    <textarea
      {...textareaProps}
      className={
        'w-full rounded-md border border-slate-200 bg-slate-50 px-2.5 py-2 ' +
        'text-[12px] leading-5 text-slate-800 outline-none placeholder:text-slate-400 ' +
        'focus:border-blue-400 focus:bg-white focus:ring-1 focus:ring-blue-100 ' +
        'disabled:cursor-not-allowed disabled:opacity-50 ' +
        className
      }
    />
  );
}
