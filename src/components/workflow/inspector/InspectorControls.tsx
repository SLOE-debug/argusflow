import type { ReactNode } from 'react';

import { Button, Input } from '../../ui';

/** 属性面板输入控件的统一 Tailwind 样式。 */
export const INSPECTOR_CONTROL_CLASS_NAME = [
  'w-full rounded-[4px] border border-slate-300 bg-white px-2.5 text-[12px]',
  'font-normal text-slate-800 outline-none focus:border-blue-400',
  'focus:ring-1 focus:ring-blue-100 disabled:bg-slate-50 disabled:text-slate-500',
].join(' ');

/** 属性面板帮助信息的统一 Tailwind 样式。 */
export const INSPECTOR_HELP_CLASS_NAME =
  'm-0 rounded-md bg-slate-50 px-2.5 py-2 text-[11px] leading-[17px] text-slate-500';

/** 属性行使用比例列，并在极窄空间下自然换行，避免固定标签宽度挤压控件。 */
const INSPECTOR_FIELD_CLASS_NAME =
  'flex min-w-0 flex-wrap items-start gap-x-2.5 gap-y-1 text-[12px] text-slate-600';

/** 标签列随面板宽度伸缩，但不会无限占用输入区域。 */
const INSPECTOR_FIELD_LABEL_CLASS_NAME = [
  'flex min-h-8 basis-[clamp(4.75rem,27%,6.25rem)] items-center',
  'text-[11px] font-medium leading-4 text-slate-500',
].join(' ');

/** 控件列保留可操作宽度；空间不足时整体落到下一行。 */
const INSPECTOR_FIELD_CONTENT_CLASS_NAME = 'min-w-36 flex-1';

type InspectorSectionProps = Readonly<{
  /** 分段标题。 */
  title: string;
  /** 分段内容。 */
  children?: ReactNode;
  /** 与标题同级的低优先级操作。 */
  action?: ReactNode;
  /** 最后一段不绘制底边。 */
  last?: boolean;
}>;

/** 右侧属性面板中的统一分段容器。 */
export function InspectorSection({
  title,
  children,
  action,
  last = false,
}: InspectorSectionProps) {
  return (
    <section className={`px-3 py-2.5 ${last ? '' : 'border-b border-slate-200'}`}>
      {action ? (
        <div className={`${children ? 'mb-2' : ''} flex min-h-[26px] items-center gap-2`}>
          <h3 className="text-[12px] font-semibold text-slate-800">{title}</h3>
          <div className="ml-auto shrink-0">{action}</div>
        </div>
      ) : (
        <h3 className={`${children ? 'mb-2' : ''} text-[12px] font-semibold text-slate-800`}>
          {title}
        </h3>
      )}
      {children ? <div className="flex flex-col gap-1.5">{children}</div> : null}
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
    <label className={INSPECTOR_FIELD_CLASS_NAME}>
      <span className={INSPECTOR_FIELD_LABEL_CLASS_NAME}>{label}</span>
      <span className={INSPECTOR_FIELD_CONTENT_CLASS_NAME}>{children}</span>
    </label>
  );
}

/** 属性面板中的统一危险操作按钮。 */
export function InspectorDeleteButton({
  label,
  onClick,
}: Readonly<{ label: string; onClick: () => void }>) {
  return (
    <Button
      variant="danger"
      onClick={onClick}
    >
      {label}
    </Button>
  );
}

/** 在紧凑属性面板中使用短标签和独立单位展示毫秒输入。 */
export function InspectorMillisecondsField({
  label,
  ariaLabel,
  min,
  max,
  value,
  onChange,
}: Readonly<{
  label: string;
  ariaLabel: string;
  min: number;
  max: number;
  value: number;
  onChange: (value: number) => void;
}>) {
  return (
    <InspectorField label={label}>
      <Input
        aria-label={ariaLabel}
        type="number"
        min={min}
        max={max}
        value={value}
        endAdornment={<span className="text-[10px] text-slate-400">毫秒</span>}
        containerClassName="border-slate-300 bg-white"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </InspectorField>
  );
}
