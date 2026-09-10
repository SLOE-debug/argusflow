import type { ButtonHTMLAttributes } from 'react';

/** 通用按钮，业务只通过属性和内容组合。 */
export function Button({ className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className={`rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium
        text-slate-700 transition hover:bg-slate-100 focus-visible:outline-2
        focus-visible:outline-offset-2 focus-visible:outline-blue-600 disabled:cursor-not-allowed
        disabled:opacity-40 ${className}`}
      {...props}
    />
  );
}
