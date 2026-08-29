import { AlertTriangle, CheckCircle2, LoaderCircle } from 'lucide-react';

import type {
  AqlDiagnostic,
  AqlQuery,
  AutomationTarget,
  BackendKind,
  QueryCost,
  QuerySupportLevel,
} from '../../../../features/workflow';
import { diagnosticLabel } from '../../../../features/aql-editor/language/messages';
import { useLanguageDocument } from '../../../../features/aql-editor/language/useLanguageDocument';
import { useAqlInspection } from '../../../../features/workflow';
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

/** 使用普通 React 内容展示 AQL 状态，选中节点时不启动 Monaco。 */
export function AqlFieldSummary({ query, target, onEdit }: AqlFieldSummaryProps) {
  const languageState = useLanguageDocument(query.source);
  const plannerState = useAqlInspection(target);
  const diagnostics = resolveDiagnostics(languageState, plannerState);
  const firstError = diagnostics.find((diagnostic) => diagnostic.severity === 'error');

  return (
    <StructuredFieldSummary
      title="查找条件"
      badge="AQL"
      status={resolveStatus(languageState, plannerState, diagnostics, firstError)}
      preview={query.source}
      metadata={resolvePlannerSummary(plannerState)}
      actionLabel={firstError ? '修改条件' : '编辑条件'}
      onEdit={onEdit}
    />
  );
}

/** 将语言服务阶段与首个错误转换为明确状态。 */
function resolveStatus(
  state: ReturnType<typeof useLanguageDocument>,
  plannerState: ReturnType<typeof useAqlInspection>,
  diagnostics: readonly AqlDiagnostic[],
  firstError: AqlDiagnostic | undefined,
) {
  if (state.phase === 'loading') {
    return (
      <span className="flex items-center gap-1.5 text-slate-500">
        <LoaderCircle className="size-3 shrink-0 animate-spin" aria-hidden="true" />
        正在检查查找条件…
      </span>
    );
  }
  if (state.phase === 'unavailable') {
    return (
      <span className="flex items-start gap-1.5 text-amber-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        查找条件暂时无法检查
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
  if (plannerState.phase === 'unavailable') {
    return (
      <span className="flex items-start gap-1.5 text-amber-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        查找条件已通过检查，但运行环境暂不可用
      </span>
    );
  }
  if (plannerState.phase === 'loading') {
    return (
      <span className="flex items-center gap-1.5 text-slate-500">
        <LoaderCircle className="size-3 shrink-0 animate-spin" aria-hidden="true" />
        查找条件已通过语法检查
      </span>
    );
  }
  if (plannerState.inspection.status === 'invalid') {
    return (
      <span className="flex items-start gap-1.5 text-rose-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        查找条件有问题，请修改后再试
      </span>
    );
  }
  if (
    plannerState.phase === 'ready'
    && plannerState.inspection.status === 'valid'
    && plannerState.inspection.planning.selected_backend === null
  ) {
    return (
      <span className="flex items-start gap-1.5 text-amber-700">
        <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
        查找条件已通过检查，但当前环境不能运行
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5 text-emerald-700">
      <CheckCircle2 className="size-3 shrink-0" aria-hidden="true" />
      {diagnostics.length > 0
        ? `查找条件可以使用，还有 ${diagnostics.length} 条提示`
        : '查找条件可以使用'}
    </span>
  );
}

/** 从真实 Planner 选择结果中提取 Inspector 所需的一行摘要。 */
function resolvePlannerSummary(
  state: ReturnType<typeof useAqlInspection>,
): string | null {
  if (state.phase === 'loading') {
    return '正在检查运行环境…';
  }
  if (state.phase === 'unavailable') {
    return state.message;
  }
  const inspection = state.inspection;
  if (inspection.status !== 'valid') {
    return null;
  }
  const selected = inspection.planning.candidates.find(
    (candidate) => candidate.backend === inspection.planning.selected_backend,
  );
  return selected
    ? `执行方式：${BACKEND_LABELS[selected.backend]} · ${SUPPORT_LABELS[selected.support]} · ${COST_LABELS[selected.cost]}`
    : '当前环境暂时不能运行此查找';
}

/** 合并语法检查与运行环境检查，避免只显示其中一类问题。 */
function resolveDiagnostics(
  languageState: ReturnType<typeof useLanguageDocument>,
  plannerState: ReturnType<typeof useAqlInspection>,
): readonly AqlDiagnostic[] {
  const languageDiagnostics = languageState.phase === 'ready'
    ? languageState.document.parsed.diagnostics
    : [];
  const plannerDiagnostics = plannerState.phase === 'ready'
    ? plannerState.inspection.diagnostics
    : [];
  const known = new Set(languageDiagnostics.map(diagnosticKey));
  return [
    ...languageDiagnostics,
    ...plannerDiagnostics.filter((diagnostic) => !known.has(diagnosticKey(diagnostic))),
  ];
}

/** 用位置和类型去重同一条语法或规划提示。 */
function diagnosticKey(diagnostic: AqlDiagnostic): string {
  const position = diagnostic.range?.start;
  return `${diagnostic.code}-${diagnostic.backend ?? 'all'}-${position?.line ?? -1}-${position?.utf16_column ?? -1}`;
}
