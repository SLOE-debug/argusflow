import type { ReactNode } from 'react';

/** 属性面板输入控件的统一 Tailwind 样式。 */
export const INSPECTOR_CONTROL_CLASS_NAME = [
  'w-full rounded-[4px] border border-slate-300 bg-white px-2.5 text-[12px]',
  'font-normal text-slate-800 outline-none focus:border-blue-400',
  'focus:ring-1 focus:ring-blue-100 disabled:bg-slate-50 disabled:text-slate-500',
].join(' ');

/** 属性面板帮助信息的统一 Tailwind 样式。 */
export const INSPECTOR_HELP_CLASS_NAME =
  'm-0 rounded-md bg-slate-50 px-3 py-2 text-[11px] leading-[18px] text-slate-500';

type InspectorSectionProps = Readonly<{
  /** 分段标题。 */
  title: string;
  /** 分段内容。 */
  children: ReactNode;
  /** 最后一段不绘制底边。 */
  last?: boolean;
}>;

/** 右侧属性面板中的统一分段容器。 */
export function InspectorSection({ title, children, last = false }: InspectorSectionProps) {
  return (
    <section className={`px-3 py-3 ${last ? '' : 'border-b border-slate-200'}`}>
      <h3 className="mb-3 text-[13px] font-semibold text-slate-800">{title}</h3>
      <div className="flex flex-col gap-2.5">{children}</div>
    </section>
  );
}

type InspectorFieldProps = Readonly<{
  /** 字段标签。 */
  label: string;
  /** 输入控件或说明内容。 */
  children: ReactNode;
}>;

/** 适配收窄属性面板的紧凑标签列。 */
export function InspectorField({ label, children }: InspectorFieldProps) {
  return (
    <label className="grid grid-cols-[80px_minmax(0,1fr)] items-start gap-2 text-[12px] text-slate-600">
      <span className="pt-2">{label}</span>
      <span className="min-w-0">{children}</span>
    </label>
  );
}

/** 属性面板中的统一危险操作按钮。 */
export function InspectorDeleteButton({
  label,
  onClick,
}: Readonly<{ label: string; onClick: () => void }>) {
  return (
    <button
      type="button"
      className="flex h-8 items-center justify-center rounded-[4px] border border-rose-200 bg-rose-50 text-[12px] font-semibold text-rose-700 hover:bg-rose-100"
      onClick={onClick}
    >
      {label}
    </button>
  );
}
