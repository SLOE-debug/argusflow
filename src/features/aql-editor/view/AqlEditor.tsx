import {
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
  InspectorEditorSection,
} from '../../../components/ui';
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
  const formattedSource = languageDocument?.formatted_source ?? null;

  /** 将当前 WASM 实例安装到 Monaco 全局 AQL provider。 */
  const configureLanguage = useCallback((monaco: MonacoApi) => {
    registerAqlMonacoLanguage(
      monaco,
      languageState.phase === 'ready' ? languageState.service : null,
    );
  }, [languageState]);

  /** 标题栏只保留对当前 AQL 文档直接生效的命令。 */
  const editorActions = (
    <button
      type="button"
      className="flex h-7 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600 hover:border-blue-300 hover:text-blue-700 disabled:cursor-not-allowed disabled:opacity-40"
      disabled={!formattedSource || formattedSource === query.source}
      onClick={() => {
        if (formattedSource) {
          editorRef.current?.replaceAll(formattedSource);
        }
      }}
    >
      <WandSparkles className="size-3" aria-hidden="true" />
      格式化
    </button>
  );

  /** 语言诊断和 Planner Explain 作为只读反馈，不参与 Monaco 文档状态。 */
  const editorFooter = (
    <>
      {languageState.phase === 'loading' ? (
        <p className="flex items-center gap-1.5 text-[10px] text-slate-500" role="status">
          <LoaderCircle className="size-3 animate-spin" aria-hidden="true" />
          正在启动 AQL 语言服务…
        </p>
      ) : null}
      {languageState.phase === 'unavailable' ? (
        <p className="rounded-md bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700">
          AQL 本地语言服务不可用；请先生成 Rust WASM 资源。{languageState.message}
        </p>
      ) : null}
      {diagnostics.some((diagnostic) => diagnostic.severity === 'error') ? (
        <DiagnosticPopup diagnostics={diagnostics} />
      ) : (
        <p className="flex items-center gap-1.5 text-[10px] text-emerald-700" role="status">
          <CheckCircle2 className="size-3" aria-hidden="true" />
          查询可用
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
    <InspectorEditorSection
      title="查找规则"
      badge="AQL"
      actions={editorActions}
      footer={editorFooter}
      renderContent={(layout) => (
        <MonacoEditor
          ref={editorRef}
          ariaLabel="AQL 查询"
          value={query.source}
          language={AQL_LANGUAGE_ID}
          modelUri={modelUri}
          className={layout === 'expanded' ? 'h-[calc(100vh-220px)] min-h-[420px]' : 'h-[220px]'}
          configure={configureLanguage}
          options={AQL_EDITOR_OPTIONS}
          onChange={(source) => onChange({ ...query, source })}
        />
      )}
    />
  );
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
