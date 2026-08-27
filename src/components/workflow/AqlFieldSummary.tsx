import { AlertTriangle, CheckCircle2, LoaderCircle } from 'lucide-react';

import type {
  AqlDiagnostic,
  AqlQuery,
  AutomationTarget,
  BackendKind,
  QueryCost,
  QuerySupportLevel,
} from '../../features/workflow/contracts';
import { diagnosticLabel } from '../../features/aql-editor/language/messages';
import { useLanguageDocument } from '../../features/aql-editor/language/useLanguageDocument';
import { useAqlInspection } from '../../features/workflow/useAqlInspection';
import { StructuredFieldSummary } from './StructuredFieldSummary';

type AqlFieldSummaryProps = Readonly<{
  /** 当前持久化的 AQL 文档。 */
  query: AqlQuery;
  /** Planner 评估所需的完整目标。 */
  target: AutomationTarget;
  /** 请求在中央工作区编辑当前规则。 */
  onEdit: () => void;
}>;

const BACKEND_LABELS: Readonly<Record<BackendKind, string>> = {
  windows_uia: 'Windows UI',
  browser_cdp: '浏览器',
  visual_cache: '视觉缓存',
  ocr_tiny: '轻量 OCR',
  ocr_medium: 'OCR',
  gui_grounding: '视觉定位',
  send_input: '坐标输入',
};

const SUPPORT_LABELS: Readonly<Record<QuerySupportLevel, string>> = {
  native: '直接支持',
  hybrid: '额外筛选',
  emulated: '模拟执行',
  unsupported: '不支持',
};

const COST_LABELS: Readonly<Record<QueryCost, string>> = {
  low: '低开销',
  medium: '中等开销',
  high: '高开销',
};

/** 使用普通 React 内容展示 AQL 状态，选中节点时不启动 Monaco。 */
export function AqlFieldSummary({ query, target, onEdit }: AqlFieldSummaryProps) {
  const languageState = useLanguageDocument(query.source);
  const plannerState = useAqlInspection(target);
  const diagnostics = languageState.phase === 'ready'
    ? languageState.document.parsed.diagnostics
    : plannerState.phase === 'ready'
      ? plannerState.inspection.diagnostics
      : [];
  const firstError = diagnostics.find((diagnostic) => diagnostic.severity === 'error');

  return (
    <StructuredFieldSummary
      title="查找规则"
      badge="AQL"
      status={resolveStatus(languageState, diagnostics, firstError)}
      preview={query.source}
      metadata={resolvePlannerSummary(plannerState)}
      actionLabel={firstError ? '修复规则' : '编辑规则'}
      onEdit={onEdit}
    />
  );
}

/** 将语言服务阶段与首个错误转换为明确状态。 */
function resolveStatus(
  state: ReturnType<typeof useLanguageDocument>,
  diagnostics: readonly AqlDiagnostic[],
  firstError: AqlDiagnostic | undefined,
) {
  if (state.phase === 'loading') {
    return (
      <span className="flex items-center gap-1.5 text-slate-500">
        <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
        正在检查查询…
      </span>
    );
  }
  if (state.phase === 'unavailable') {
    return (
      <span className="flex items-start gap-1.5 text-amber-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        AQL 语言服务不可用
      </span>
    );
  }
  if (firstError) {
    const position = firstError.range?.start;
    return (
      <span className="flex items-start gap-1.5 text-rose-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        <span>
          {position ? `第 ${position.line + 1} 行：` : ''}{diagnosticLabel(firstError)}
        </span>
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5 text-emerald-700">
      <CheckCircle2 className="size-3" aria-hidden="true" />
      {diagnostics.length > 0 ? `查询可用，${diagnostics.length} 条提示` : '查询可用'}
    </span>
  );
}

/** 从真实 Planner 选择结果中提取 Inspector 所需的一行摘要。 */
function resolvePlannerSummary(
  state: ReturnType<typeof useAqlInspection>,
): string | null {
  if (state.phase === 'loading') {
    return '正在评估当前执行环境…';
  }
  if (state.phase === 'unavailable') {
    return state.message;
  }
  if (state.inspection.status !== 'valid') {
    return null;
  }
  const selected = state.inspection.planning.candidates.find(
    (candidate) => candidate.backend === state.inspection.planning.selected_backend,
  );
  return selected
    ? `${BACKEND_LABELS[selected.backend]} · ${SUPPORT_LABELS[selected.support]} · ${COST_LABELS[selected.cost]}`
    : '当前没有可执行方式';
}
