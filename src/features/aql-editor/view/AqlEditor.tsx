import LoaderCircle from 'lucide-react/dist/esm/icons/loader-circle.mjs';
import WandSparkles from 'lucide-react/dist/esm/icons/wand-sparkles.mjs';
import {
  useCallback,
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
} from '../../workflow';
import {
  AQL_LANGUAGE_ID,
  registerAqlMonacoLanguage,
} from '../language/MonacoAqlLanguage';
import { useLanguageDocument } from '../language/useLanguageDocument';
import { DiagnosticPopup } from './DiagnosticPopup';

type AqlEditorProps = Readonly<{
  /** 当前节点持久化的版本化 AQL。 */
  query: AqlQuery;
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
export function AqlEditor({ query, modelUri, onChange }: AqlEditorProps) {
  const editorRef = useRef<MonacoEditorHandle>(null);
  const languageState = useLanguageDocument(query.source);
  const languageDocument = languageState.phase === 'ready' ? languageState.document : null;
  /** 编辑期只展示语言服务诊断，不推测运行时窗口或后端可用性。 */
  const diagnostics = languageDocument?.parsed.diagnostics ?? [];
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
  const editorActions = formatAvailability.type !== 'clean' ? (
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
        <LoaderCircle className="size-3 shrink-0 animate-spin" aria-hidden="true" />
      ) : (
        <WandSparkles className="size-3 shrink-0" aria-hidden="true" />
      )}
      {formatAvailability.label}
    </button>
  ) : null;

  /** 语言诊断作为只读反馈，不参与 Monaco 文档状态。 */
  const editorFooter = (
    <>
      {languageState.phase === 'loading' ? (
        <p className="flex items-center gap-1.5 text-[10px] text-slate-500" role="status">
          <LoaderCircle className="size-3 shrink-0 animate-spin" aria-hidden="true" />
          正在检查查找条件…
        </p>
      ) : null}
      {languageState.phase === 'unavailable' ? (
        <p className="rounded-md bg-amber-50 px-2.5 py-2 text-[10px] leading-4 text-amber-700">
          查找条件暂时无法检查，请稍后重试。
        </p>
      ) : null}
      {diagnostics.length > 0 ? <DiagnosticPopup diagnostics={diagnostics} /> : null}
    </>
  );
  /** 没有诊断时不给编辑器保留空白反馈栏。 */
  const footerVisible = languageState.phase !== 'ready' || diagnostics.length > 0;

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
      {footerVisible ? (
        <div className="max-h-[38%] shrink-0 overflow-y-auto border-t border-slate-200 px-3 py-2">
          <div className="flex flex-col gap-2">{editorFooter}</div>
        </div>
      ) : null}
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
