import Pencil from 'lucide-react/dist/esm/icons/pencil.mjs';
import type { ReactNode } from 'react';

type StructuredFieldSummaryProps = Readonly<{
  /** 文档字段名称。 */
  title: string;
  /** 文档语言或格式。 */
  badge: string;
  /** 诊断、来源或行数等简短状态。 */
  status: ReactNode;
  /** 不可编辑的轻量源码预览。 */
  preview: string;
  /** 可选的规划或运行能力摘要。 */
  metadata?: string | null;
  /** 进入中央工作区的产品动作名称。 */
  actionLabel: string;
  /** 请求 Workspace 打开对应结构化文档。 */
  onEdit: () => void;
}>;

/** Inspector 中只读、无 Monaco 实例的结构化文档摘要卡。 */
export function StructuredFieldSummary({
  title,
  badge,
  status,
  preview,
  metadata = null,
  actionLabel,
  onEdit,
}: StructuredFieldSummaryProps) {
  return (
    <section className="overflow-hidden rounded-lg border border-slate-200 bg-slate-50/60 shadow-sm">
      <button
        type="button"
        className="flex w-full flex-col gap-2.5 p-2.5 text-left hover:bg-blue-50/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-500"
        aria-label={actionLabel}
        onClick={onEdit}
      >
        <span className="flex w-full items-center gap-2">
          <h3 className="text-[12px] font-semibold text-slate-800">{title}</h3>
          <span className="rounded bg-slate-200/70 px-1.5 py-0.5 font-mono text-[9px] font-medium text-slate-500">
            {badge}
          </span>
        </span>
        <span className="w-full text-[10px] leading-4 text-slate-600">{status}</span>
        <code className="line-clamp-3 w-full whitespace-pre-wrap break-all rounded-md border border-slate-200 bg-white px-2.5 py-2 font-mono text-[10px] leading-4 text-slate-700">
          {preview || '（暂无内容）'}
        </code>
        <span className="flex w-full items-center gap-2">
          {metadata ? (
            <span className="min-w-0 flex-1 truncate text-[9px] text-slate-500">
              {metadata}
            </span>
          ) : <span className="flex-1" />}
          <span className="flex shrink-0 items-center gap-1 text-[10px] font-semibold text-blue-600">
            <Pencil className="size-3 shrink-0" aria-hidden="true" />
            {actionLabel}
          </span>
        </span>
      </button>
    </section>
  );
}
