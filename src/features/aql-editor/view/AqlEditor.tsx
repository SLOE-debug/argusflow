import {
  AlertTriangle,
  CheckCircle2,
  LoaderCircle,
  WandSparkles,
} from 'lucide-react';
import {
  useCallback,
  useMemo,
  useRef,
} from 'react';
import type * as Monaco from 'monaco-editor/editor/editor.api';

import {
  MonacoEditor,
  type MonacoApi,
  type MonacoEditorHandle,
} from '../../../components/ui/monaco';
import type {
  AqlDiagnostic,
  AqlQuery,
  AutomationTarget,
} from '../../workflow/contracts';
import { useAqlInspection } from '../../workflow/useAqlInspection';
import {
  AQL_LANGUAGE_ID,
  registerAqlMonacoLanguage,
} from '../language/MonacoAqlLanguage';
import { useLanguageDocument } from '../language/useLanguageDocument';
import { DiagnosticPopup } from './DiagnosticPopup';
import { PlanExplanation } from './PlanExplanation';

type AqlEditorProps = Readonly<{
  /** 当前节点持久化的版本化 AQL。 */
  query: AqlQuery;
  /** Runtime Planner 使用的完整目标作用域与后端约束。 */
  target: AutomationTarget;
  /** 当前字段稳定且唯一的 Monaco 模型 URI。 */
  modelUri: string;
  /** 写回节点的 AQL 源码。 */
  onChange: (query: AqlQuery) => void;
}>;

/** Toolbar 对格式化命令可用性的明确说明。 */
type FormatAvailability =
  | { readonly type: 'loading'; readonly label: '正在检查查找条件' }
  | { readonly type: 'invalid'; readonly label: string }
  | { readonly type: 'clean'; readonly label: '已格式化' }
  | { readonly type: 'dirty'; readonly label: '格式化' };

/** AQL 编辑器覆盖项；Hover 延迟与当前 VS Code 默认值保持一致。 */
const AQL_EDITOR_OPTIONS = {
  bracketPairColorization: { enabled: true },
  folding: false,
  glyphMargin: false,
  guides: { bracketPairs: true },
  hover: { enabled: 'on', delay: 300, sticky: true, hidingDelay: 300 },
  lineNumbers: 'on',
  lineNumbersMinChars: 3,
  quickSuggestions: { other: true, comments: false, strings: false },
  suggestOnTriggerCharacters: true,
  wordWrap: 'off',
} as const satisfies Monaco.editor.IStandaloneEditorConstructionOptions;

/** 使用 Monaco 承载编辑交互，并由 Rust/WASM 注册 AQL 语言能力。 */
export function AqlEditor({ query, target, modelUri, onChange }: AqlEditorProps) {
  const editorRef = useRef<MonacoEditorHandle>(null);
  const plannerState = useAqlInspection(target);
  const languageState = useLanguageDocument(query.source);
  const languageDocument = languageState.phase === 'ready' ? languageState.document : null;
  const diagnostics = useMemo(
    () => resolveDiagnostics(languageDocument?.parsed.diagnostics ?? null, plannerState),
    [languageDocument, plannerState],
  );
  const formatAvailability = resolveFormatAvailability(
    query.source,
    languageState,
    languageDocument?.parsed.diagnostics ?? [],
  );

  /** 将当前 WASM 实例安装到 Monaco 全局 AQL provider。 */
  const configureLanguage = useCallback((monaco: MonacoApi) => {
    registerAqlMonacoLanguage(
      monaco,
      languageState.phase === 'ready' ? languageState.service : null,
    );
  }, [languageState]);

  /** Toolbar 与 Shift+Alt+F 都进入 Monaco 的同一个 Format Document 动作。 */
  const editorActions = formatAvailability.type === 'clean' ? (
    <span className="flex h-7 items-center gap-1 px-2 text-[10px] font-medium text-emerald-700">
      <CheckCircle2 className="size-3" aria-hidden="true" />
      {formatAvailability.label}
    </span>
  ) : (
    <button
      type="button"
      className="flex h-7 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600 hover:border-blue-300 hover:text-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
      disabled={formatAvailability.type !== 'dirty'}
      title={formatAvailability.type === 'invalid'
        ? formatAvailability.label
        : formatAvailability.type === 'loading'
          ? '正在检查查找条件'
          : '整理查找条件'}
      onClick={() => void editorRef.current?.formatDocument()}
    >
      {formatAvailability.type === 'loading' ? (
        <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
      ) : (
        <WandSparkles className="size-3" aria-hidden="true" />
      )}
      {formatAvailability.label}
    </button>
  );

  /** 语言诊断和 Planner Explain 作为只读反馈，不参与 Monaco 文档状态。 */
  const editorFooter = (
    <>
      {languageState.phase === 'loading' ? (
        <p className="flex items-center gap-1.5 text-[10px] text-slate-500" role="status">
          <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
          正在检查查找条件…
        </p>
      ) : null}
      {languageState.phase === 'unavailable' ? (
        <p className="rounded-md bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700">
          查找条件暂时无法检查，请稍后重试。
        </p>
      ) : null}
      {diagnostics.some((diagnostic) => diagnostic.severity === 'error') ? (
        <DiagnosticPopup diagnostics={diagnostics} />
      ) : plannerState.phase === 'unavailable' ? (
        <p className="flex items-center gap-1.5 text-[10px] text-amber-700" role="status">
          <AlertTriangle className="size-3" aria-hidden="true" />
          查找条件已通过语法检查，但运行环境暂不可用
        </p>
      ) : plannerState.phase === 'ready'
        && plannerState.inspection.status === 'valid'
        && plannerState.inspection.planning.selected_backend === null ? (
        <p className="flex items-center gap-1.5 text-[10px] text-amber-700" role="status">
          <AlertTriangle className="size-3" aria-hidden="true" />
          查找条件已通过语法检查，但当前环境不能运行
        </p>
      ) : (
        <p className="flex items-center gap-1.5 text-[10px] text-emerald-700" role="status">
          <CheckCircle2 className="size-3" aria-hidden="true" />
          查找条件可以使用
        </p>
      )}
      {diagnostics.length > 0
        && diagnostics.every((diagnostic) => diagnostic.severity !== 'error') ? (
          <DiagnosticPopup diagnostics={diagnostics} />
        ) : null}
      <PlanExplanation state={plannerState} />
    </>
  );

  return (
    <section className="flex h-full min-h-0 flex-col bg-white">
      <div className="flex h-9 shrink-0 items-center justify-end border-b border-slate-200 bg-slate-50/70 px-2">
        {editorActions}
      </div>
      <div className="min-h-0 flex-1 p-2">
        <MonacoEditor
          ref={editorRef}
          ariaLabel="AQL 查找条件"
          value={query.source}
          language={AQL_LANGUAGE_ID}
          modelUri={modelUri}
          className="h-full min-h-0"
          configure={configureLanguage}
          options={AQL_EDITOR_OPTIONS}
          onChange={(source) => onChange({ ...query, source })}
        />
      </div>
      <div className="max-h-[38%] shrink-0 overflow-y-auto border-t border-slate-200 px-3 py-2">
        <div className="flex flex-col gap-2">{editorFooter}</div>
      </div>
    </section>
  );
}

/** 根据语言服务、诊断和 formatter 结果建立可解释的格式化状态。 */
function resolveFormatAvailability(
  source: string,
  languageState: ReturnType<typeof useLanguageDocument>,
  diagnostics: readonly AqlDiagnostic[],
): FormatAvailability {
  if (languageState.phase === 'loading') {
    return { type: 'loading', label: '正在检查查找条件' };
  }
  if (languageState.phase === 'unavailable') {
    return { type: 'invalid', label: '查找条件暂时无法检查' };
  }
  if (diagnostics.some((diagnostic) => diagnostic.severity === 'error')) {
    return { type: 'invalid', label: '请先修复语法错误' };
  }
  const formattedSource = languageState.document.formatted_source;
  if (!formattedSource) {
    return { type: 'invalid', label: '当前文档无法格式化' };
  }
  return formattedSource === source
    ? { type: 'clean', label: '已格式化' }
    : { type: 'dirty', label: '格式化' };
}

/** WASM 诊断优先；WASM 尚未就绪时使用 Runtime recovery parser 的结果。 */
function resolveDiagnostics(
  languageDiagnostics: readonly AqlDiagnostic[] | null,
  plannerState: ReturnType<typeof useAqlInspection>,
): readonly AqlDiagnostic[] {
  const plannerDiagnostics = plannerState.phase === 'ready'
    ? plannerState.inspection.diagnostics
    : [];
  if (!languageDiagnostics) {
    return plannerDiagnostics;
  }
  const languageKeys = new Set(languageDiagnostics.map(diagnosticKey));
  return [
    ...languageDiagnostics,
    ...plannerDiagnostics.filter((diagnostic) => !languageKeys.has(diagnosticKey(diagnostic))),
  ];
}

/** 构造诊断去重键，避免 WASM 与 IPC recovery parser 重复展示同一问题。 */
function diagnosticKey(diagnostic: AqlDiagnostic): string {
  const position = diagnostic.range?.start;
  return `${diagnostic.code}-${diagnostic.backend ?? 'all'}-${position?.line ?? -1}-${position?.utf16_column ?? -1}`;
}
