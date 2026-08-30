import AlertTriangle from 'lucide-react/dist/esm/icons/triangle-alert.mjs';
import CheckCircle2 from 'lucide-react/dist/esm/icons/circle-check.mjs';
import LoaderCircle from 'lucide-react/dist/esm/icons/loader-circle.mjs';

import type {
  AqlInspection,
  BackendKind,
  PlanStepKind,
  QueryCost,
  QuerySupportLevel,
  RuntimeAvailability,
} from '../../workflow';
import type { AqlInspectionState } from '../../workflow';
import { diagnosticLabel } from '../language/messages';

const BACKEND_LABELS: Readonly<Record<BackendKind, string>> = {
  windows_uia: 'Windows UI 自动化',
  browser_cdp: '浏览器自动化',
  ocr_small: '桌面文字识别',
  send_input: '模拟输入',
};

const SUPPORT_LABELS: Readonly<Record<QuerySupportLevel, string>> = {
  native: '可直接执行',
  hybrid: '可兼容执行',
  emulated: '需要逐个查找',
  unsupported: '暂不支持',
};

const COST_LABELS: Readonly<Record<QueryCost, string>> = {
  low: '速度快',
  medium: '速度一般',
  high: '速度可能较慢',
};

const AVAILABILITY_LABELS: Readonly<Record<RuntimeAvailability, string>> = {
  ready: '可执行',
  missing_context: '缺少必要环境',
  unavailable: '暂不可用',
  not_implemented: '暂不支持',
};

/** 将技术步骤类型翻译成可展开查看的简短说明。 */
const PLAN_STEP_LABELS: Readonly<Record<PlanStepKind, string>> = {
  scope: '作用范围',
  candidate_source: '目标来源',
  pushdown: '快速筛选',
  cache: '读取属性',
  residual: '额外筛选',
  selection: '选择目标',
  traversal: '遍历元素',
  action: '执行操作',
};

type ValidInspection = Extract<AqlInspection, { status: 'valid' }>;

/** Runtime Planner 的只读产品摘要与开发者 Explain。 */
export function PlanExplanation({ state }: Readonly<{ state: AqlInspectionState }>) {
  if (state.phase === 'loading') {
    return (
      <p className="flex items-center gap-1.5 text-[10px] text-slate-500" role="status">
        <LoaderCircle className="size-3 shrink-0 animate-spin" aria-hidden="true" />
        正在检查运行环境…
      </p>
    );
  }
  if (state.phase === 'unavailable') {
    return (
      <p className="rounded-md bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700">
        {state.message}
      </p>
    );
  }
  if (state.inspection.status === 'invalid') {
    return null;
  }
  return <ValidPlanExplanation inspection={state.inspection} />;
}

/** 有效查询的 Planner 结果。 */
function ValidPlanExplanation({ inspection }: Readonly<{ inspection: ValidInspection }>) {
  const selected = inspection.planning.candidates.find(
    (candidate) => candidate.backend === inspection.planning.selected_backend,
  );

  return (
    <div className="rounded-md border border-slate-200 bg-white p-2.5">
      <div className="flex items-start gap-2">
        {selected ? (
          <CheckCircle2 className="mt-0.5 size-3.5 shrink-0 text-emerald-600" aria-hidden="true" />
        ) : (
          <AlertTriangle className="mt-0.5 size-3.5 shrink-0 text-amber-600" aria-hidden="true" />
        )}
        <div className="min-w-0">
          <strong className="block text-[10px] text-slate-700">
            {selected ? `执行方式：${BACKEND_LABELS[selected.backend]}` : '当前环境暂时不能运行此查找'}
          </strong>
          <span className="mt-0.5 block text-[9px] text-slate-500">
            {selected
              ? `${SUPPORT_LABELS[selected.support]}，${COST_LABELS[selected.cost]}`
              : '展开“查看详情”了解原因。'}
          </span>
        </div>
      </div>
      <details className="mt-2 border-t border-slate-100 pt-2 text-[9px] text-slate-500">
        <summary className="cursor-pointer select-none font-medium text-slate-600">
          查看详情
        </summary>
        <div className="mt-2 space-y-2">
          {inspection.planning.candidates.map((candidate) => (
            <div key={candidate.backend} className="rounded border border-slate-200 bg-slate-50 p-2">
              <div className="flex items-center justify-between gap-2">
                <strong className="text-slate-700">{BACKEND_LABELS[candidate.backend]}</strong>
                <span>{SUPPORT_LABELS[candidate.support]}</span>
              </div>
              <div className="mt-1 flex gap-2 text-slate-400">
                <span>{AVAILABILITY_LABELS[candidate.availability]}</span>
                <span>{COST_LABELS[candidate.cost]}</span>
              </div>
              {candidate.steps.length > 0 ? (
                <ul className="mt-1.5 space-y-0.5 font-mono text-[8px] leading-3.5 text-slate-500">
                  {candidate.steps.map((step, index) => (
                    <li key={`${step.kind}-${index}`}>
                      {PLAN_STEP_LABELS[step.kind]}：{step.summary}
                    </li>
                  ))}
                </ul>
              ) : null}
              {candidate.diagnostics.length > 0 ? (
                <ul className="mt-1.5 space-y-0.5 text-amber-700">
                  {candidate.diagnostics.map((diagnostic, index) => (
                    <li key={`${diagnostic.code}-${index}`}>{diagnosticLabel(diagnostic)}</li>
                  ))}
                </ul>
              ) : null}
            </div>
          ))}
          <code className="block break-all rounded bg-slate-100 p-1.5 font-mono text-[8px] leading-3.5 text-slate-600">
            {inspection.canonical_source}
          </code>
        </div>
      </details>
    </div>
  );
}
