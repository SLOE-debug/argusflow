import {
  AlertTriangle,
  CheckCircle2,
  LoaderCircle,
  WandSparkles,
} from 'lucide-react';

import type {
  AqlInspection,
  AqlQuery,
  BackendPreference,
  QueryBackend,
  QueryCost,
  QuerySupportLevel,
} from '../../features/workflow/contracts';
import { useAqlInspection } from '../../features/workflow/useAqlInspection';
import { Textarea } from '../ui';

type AqlEditorProps = Readonly<{
  /** 当前节点持久化的版本化 AQL。 */
  query: AqlQuery;
  /** 用于在能力列表中强调实际选择的后端。 */
  backendPreference: BackendPreference;
  /** 写回节点的 AQL 源码。 */
  onChange: (query: AqlQuery) => void;
}>;

/** 后端名称、支持等级与成本的稳定中文展示。 */
const BACKEND_LABELS: Readonly<Record<QueryBackend, string>> = {
  windows_uia: 'UIA',
  browser_cdp: 'CDP',
  vision: '视觉',
};

const SUPPORT_LABELS: Readonly<Record<QuerySupportLevel, string>> = {
  native: '原生',
  hybrid: '混合',
  emulated: '模拟',
  unsupported: '不支持',
};

const SUPPORT_TONES: Readonly<Record<QuerySupportLevel, string>> = {
  native: 'bg-emerald-50 text-emerald-700',
  hybrid: 'bg-blue-50 text-blue-700',
  emulated: 'bg-amber-50 text-amber-700',
  unsupported: 'bg-slate-100 text-slate-500',
};

const COST_LABELS: Readonly<Record<QueryCost, string>> = {
  low: '低成本',
  medium: '中成本',
  high: '高成本',
};

/** 带实时 Rust parser 诊断、formatter 和 capability explain 的 AQL 编辑器。 */
export function AqlEditor({ query, backendPreference, onChange }: AqlEditorProps) {
  const state = useAqlInspection(query);
  const inspection = state.phase === 'ready' ? state.inspection : null;
  const validInspection = inspection?.status === 'valid' ? inspection : null;
  const invalidInspection = inspection?.status === 'invalid' ? inspection : null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] font-semibold text-slate-700">目标查询</span>
        <button
          type="button"
          className="flex h-7 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600 hover:border-blue-300 hover:text-blue-700 disabled:cursor-not-allowed disabled:opacity-40"
          disabled={!validInspection}
          onClick={() => {
            if (validInspection) {
              onChange({ ...query, source: validInspection.formatted_source });
            }
          }}
        >
          <WandSparkles className="size-3" aria-hidden="true" />
          格式化
        </button>
      </div>
      <Textarea
        aria-label="AQL 查询"
        aria-invalid={Boolean(invalidInspection)}
        className={
          'h-[132px] resize-y font-mono text-[11px] leading-[18px] ' +
          (invalidInspection ? 'border-rose-300 focus:border-rose-400 focus:ring-rose-100' : '')
        }
        spellCheck={false}
        value={query.source}
        onChange={(event) => onChange({ ...query, source: event.target.value })}
      />
      <p className="text-[9px] leading-4 text-slate-400">
        使用 <code className="font-mono text-slate-500">&gt;</code> 表示直接子元素，
        <code className="font-mono text-slate-500">&gt;&gt;</code> 表示任意后代；
        用 <code className="font-mono text-slate-500">first(...)</code> 明确单结果。
      </p>
      <InspectionStatus state={state} />
      {validInspection ? (
        <QueryExplanation
          inspection={validInspection}
          backendPreference={backendPreference}
        />
      ) : null}
    </div>
  );
}

/** 展示解析过程、错误位置或桌面 IPC 不可用原因。 */
function InspectionStatus({
  state,
}: Readonly<{ state: ReturnType<typeof useAqlInspection> }>) {
  if (state.phase === 'loading') {
    return (
      <p className="flex items-center gap-1.5 text-[10px] text-slate-500" role="status">
        <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
        正在分析查询…
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
    const { diagnostic } = state.inspection;
    return (
      <div
        className="rounded-md border border-rose-200 bg-rose-50 px-2.5 py-2 text-[10px] leading-4 text-rose-700"
        role="alert"
      >
        <p className="flex items-start gap-1.5 font-medium">
          <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
          <span>
            第 {diagnostic.span.line} 行，第 {diagnostic.span.column} 列：
            {diagnostic.message}
          </span>
        </p>
        {diagnostic.help ? (
          <p className="mt-1 pl-[18px] text-rose-600">建议：{diagnostic.help}</p>
        ) : null}
      </div>
    );
  }

  return (
    <p className="flex items-center gap-1.5 text-[10px] text-emerald-700" role="status">
      <CheckCircle2 className="size-3" aria-hidden="true" />
      查询语法与谓词类型有效
    </p>
  );
}

type ValidInspection = Extract<AqlInspection, { status: 'valid' }>;

/** 将 portability、后端能力、成本和静态警告组织为紧凑 explain 面板。 */
function QueryExplanation({
  inspection,
  backendPreference,
}: Readonly<{
  inspection: ValidInspection;
  backendPreference: BackendPreference;
}>) {
  const preferredBackend = resolvePreferredBackend(backendPreference);
  const portabilityLabel = inspection.portability.type === 'portable'
    ? '跨后端语义'
    : `后端专用：${inspection.portability.backends
        .map((backend) => BACKEND_LABELS[backend])
        .join('、')}`;

  return (
    <div className="rounded-md border border-slate-200 bg-white p-2.5">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[10px] font-semibold text-slate-700">查询说明</span>
        <span className="rounded bg-slate-100 px-1.5 py-0.5 text-[9px] text-slate-600">
          {portabilityLabel}
        </span>
      </div>
      <div className="mt-2 grid grid-cols-3 gap-1.5">
        {inspection.capabilities.map((capability) => {
          const preferred = capability.backend === preferredBackend;
          return (
            <div
              key={capability.backend}
              className={
                'min-w-0 rounded-md border p-1.5 ' +
                (preferred ? 'border-blue-300 bg-blue-50/40' : 'border-slate-200 bg-slate-50')
              }
            >
              <div className="flex items-center justify-between gap-1">
                <strong className="truncate text-[9px] text-slate-700">
                  {BACKEND_LABELS[capability.backend]}
                </strong>
                {preferred ? (
                  <span className="size-1.5 shrink-0 rounded-full bg-blue-500" title="指定后端" />
                ) : null}
              </div>
              <span className={`mt-1 block truncate rounded px-1 py-0.5 text-center text-[9px] ${SUPPORT_TONES[capability.level]}`}>
                {SUPPORT_LABELS[capability.level]}
              </span>
              <span className="mt-1 block text-center text-[9px] text-slate-400">
                {COST_LABELS[capability.estimated_cost]}
              </span>
            </div>
          );
        })}
      </div>
      {inspection.warnings.length > 0 ? (
        <ul className="mt-2 space-y-1 border-t border-slate-100 pt-2">
          {inspection.warnings.map((warning) => (
            <li
              key={`${warning.kind}-${warning.backend ?? 'all'}`}
              className="flex items-start gap-1.5 text-[9px] leading-4 text-amber-700"
            >
              <AlertTriangle className="mt-0.5 size-2.5 shrink-0" aria-hidden="true" />
              <span>{warning.message}</span>
            </li>
          ))}
        </ul>
      ) : null}
      <details className="mt-2 border-t border-slate-100 pt-2 text-[9px] text-slate-500">
        <summary className="cursor-pointer select-none">查看规范查询</summary>
        <code className="mt-1 block break-all rounded bg-slate-50 p-1.5 font-mono leading-4 text-slate-600">
          {inspection.canonical_source}
        </code>
      </details>
    </div>
  );
}

/** 将用户偏好映射到 analyzer 后端；自动模式不强调单个计划。 */
function resolvePreferredBackend(preference: BackendPreference): QueryBackend | null {
  switch (preference) {
    case 'auto':
      return null;
    case 'windows_uia':
      return 'windows_uia';
    case 'browser_cdp':
      return 'browser_cdp';
  }
}
