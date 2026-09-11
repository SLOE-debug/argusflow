import { useEffect, useRef } from 'react';
import type * as Monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import type { Diagnostic, LanguageService } from '../../../features/aql';
import { registerLanguage, LANGUAGE_ID } from './language';
import { bindEditorTheme } from './theme';

/** Monaco 实例的值和生命周期由单一组件维护。 */
export function CodeEditor({ source, service, diagnostics, onChange, onComposition, onError, compact = false, readOnly = false }: {
  readonly compact?: boolean;
  readonly readOnly?: boolean;
  readonly source: string;
  readonly service: LanguageService;
  readonly diagnostics: readonly Diagnostic[];
  readonly onChange: (source: string) => void;
  readonly onComposition: (active: boolean) => void;
  readonly onError: () => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<Monaco.editor.IStandaloneCodeEditor | undefined>(undefined);
  const monacoApi = useRef<typeof Monaco | undefined>(undefined);
  const callbacks = useRef({ onChange, onComposition, onError });
  callbacks.current = { onChange, onComposition, onError };
  const initial = useRef(source);
  const currentDiagnostics = useRef(diagnostics);
  currentDiagnostics.current = diagnostics;

  useEffect(() => {
    let cancelled = false;
    let dispose: (() => void) | undefined;
    void import('./runtime').then(({ monaco }) => {
      if (cancelled || !host.current) return;
      monacoApi.current = monaco;
      const releaseTheme = bindEditorTheme(monaco);
      let composing = false;
      const language = registerLanguage(monaco, service, () => composing);
      const model = monaco.editor.createModel(initial.current, LANGUAGE_ID);
      const instance = monaco.editor.create(host.current, {
        model, ariaLabel: '中文 AQL 查询', automaticLayout: true, minimap: { enabled: false },
        fontSize: compact ? 13 : 16, lineHeight: compact ? 22 : 28, padding: { top: 12, bottom: 12 }, scrollBeyondLastLine: false,
        readOnly,
        tabSize: 2, wordWrap: 'on', overviewRulerLanes: 0, contextmenu: true,
        // 原生悬浮和候选浮层按视口定位，避免被圆角卡片的 overflow-hidden 裁切。
        fixedOverflowWidgets: true,
        hover: { enabled: true, delay: 300 },
        quickSuggestions: { other: 'on', comments: 'off', strings: 'off' },
        suggestOnTriggerCharacters: true, quickSuggestionsDelay: 100,
        wordBasedSuggestions: 'off', tabCompletion: 'on',
      });
      editor.current = instance;
      markDiagnostics(monaco, model, currentDiagnostics.current);
      const changed = instance.onDidChangeModelContent(() => callbacks.current.onChange(instance.getValue()));
      const start = instance.onDidCompositionStart(() => { composing = true; callbacks.current.onComposition(true); });
      const end = instance.onDidCompositionEnd(() => {
        composing = false;
        callbacks.current.onComposition(false);
        // 等 Monaco 提交输入法文本，再对已经完成的关键字前缀请求候选。
        queueMicrotask(() => {
          const position = instance.getPosition();
          if (editor.current === instance && !composing && position && model.getWordUntilPosition(position).word) {
            instance.trigger('aql-composition', 'editor.action.triggerSuggest', {});
          }
        });
      });
      dispose = () => {
        changed.dispose(); start.dispose(); end.dispose(); instance.dispose(); model.dispose(); language.dispose();
        editor.current = undefined;
        releaseTheme();
      };
    }).catch(() => callbacks.current.onError());
    return () => { cancelled = true; dispose?.(); };
  }, [service]);
  useEffect(() => { editor.current?.updateOptions({ readOnly }); }, [readOnly]);

  useEffect(() => {
    initial.current = source;
    const instance = editor.current;
    if (instance && instance.getValue() !== source) {
      const model = instance.getModel();
      if (model) instance.executeEdits('aql-replace', [{ range: model.getFullModelRange(), text: source }]);
    }
  }, [source]);

  useEffect(() => {
    const model = editor.current?.getModel();
    const monaco = monacoApi.current;
    if (!model || !monaco) return;
    markDiagnostics(monaco, model, diagnostics);
  }, [diagnostics]);

  return <div
    ref={host}
    className={compact ? 'min-h-0 min-w-0 flex-1' : 'h-[420px] min-w-0'}
    aria-label="中文查询编辑区"
  />;
}

function markDiagnostics(monaco: typeof Monaco, model: Monaco.editor.ITextModel, diagnostics: readonly Diagnostic[]) {
  monaco.editor.setModelMarkers(model, LANGUAGE_ID, diagnostics.map((item) => ({
    severity: monaco.MarkerSeverity.Error, message: item.message,
    startLineNumber: item.range.start.line + 1, startColumn: item.range.start.column + 1,
    endLineNumber: item.range.end.line + 1, endColumn: item.range.end.column + 1,
  })));
}
