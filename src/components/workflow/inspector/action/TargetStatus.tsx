import Circle from 'lucide-react/dist/esm/icons/circle.mjs';
import CircleAlert from 'lucide-react/dist/esm/icons/circle-alert.mjs';
import CircleCheck from 'lucide-react/dist/esm/icons/circle-check.mjs';

import type { ActionTargetStatus } from '../../../../features/workflow';

/** 展示配置状态或未来目标检查返回的 0/1/多匹配结果。 */
export function TargetStatus({ status }: Readonly<{ status: ActionTargetStatus }>) {
  /** 没有真实检查结果时不展示占位状态，避免把未实现能力伪装成用户信息。 */
  if (status.type === 'unchecked') return null;
  const presentation = resolveTargetStatusPresentation(status);
  const Icon = presentation.icon;
  return (
    <div
      className={`flex items-start gap-2 rounded-md border px-2.5 py-2 text-[11px] leading-4 ${presentation.className}`}
      role={status.type === 'invalid' ? 'alert' : 'status'}
    >
      <Icon className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
      <span>{presentation.message}</span>
    </div>
  );
}

/** 状态颜色始终与图标和文字同时出现，不要求用户单独记忆颜色。 */
function resolveTargetStatusPresentation(
  status: Exclude<ActionTargetStatus, Readonly<{ type: 'unchecked' }>>,
): Readonly<{
  icon: typeof Circle;
  message: string;
  className: string;
}> {
  switch (status.type) {
    case 'configured':
      return {
        icon: CircleCheck,
        message: status.message,
        className: 'border-emerald-200 bg-emerald-50 text-emerald-700',
      };
    case 'invalid':
      return {
        icon: CircleAlert,
        message: status.message,
        className: 'border-rose-200 bg-rose-50 text-rose-700',
      };
    case 'matched':
      if (status.count === 1) {
        return {
          icon: CircleCheck,
          message: '当前找到 1 个目标',
          className: 'border-emerald-200 bg-emerald-50 text-emerald-700',
        };
      }
      if (status.count > 1) {
        return {
          icon: CircleAlert,
          message: `当前找到 ${status.count} 个目标，需要增加条件`,
          className: 'border-amber-200 bg-amber-50 text-amber-800',
        };
      }
      return {
        icon: Circle,
        message: '当前未检测到目标',
        className: 'border-slate-200 bg-slate-50 text-slate-600',
      };
  }
}
