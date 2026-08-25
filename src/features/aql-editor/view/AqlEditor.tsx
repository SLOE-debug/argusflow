import {
  CheckCircle2,
  LoaderCircle,
  WandSparkles,
  Wrench,
} from 'lucide-react';
import {
  useMemo,
  useRef,
  useState,
} from 'react';
import type {
  ChangeEvent,
  KeyboardEvent,
  UIEvent,
} from 'react';

import type {
  AqlDiagnostic,
  AqlQuery,
  AutomationTarget,
  EditorRange,
} from '../../workflow/contracts';
import { useAqlInspection } from '../../workflow/useAqlInspection';
import { insertParenthesisPair, skipClosingParenthesis } from '../commands/bracket';
import { indentSelection, insertIndentedLine } from '../commands/indent';
import { AqlDocument } from '../core/AqlDocument';
import { EditorHistory } from '../core/History';
import { LineIndex } from '../core/LineIndex';
import type { EditorSelection } from '../core/Selection';
import { toOffsetEdit } from '../core/TextEdit';
import { useLanguageDocument } from '../language/useLanguageDocument';
import type { CompletionItem, Hover } from '../language/types';
import { CompletionPopup } from './CompletionPopup';
import { BracketLayer } from './BracketLayer';
import { DecorationLayer } from './DecorationLayer';
import { DiagnosticPopup } from './DiagnosticPopup';
import { Gutter } from './Gutter';
import { HighlightLayer } from './HighlightLayer';
import { HoverPopup } from './HoverPopup';
import { InputLayer } from './InputLayer';
import { PlanExplanation } from './PlanExplanation';

type AqlEditorProps = Readonly<{
  /** 当前节点持久化的版本化 AQL。 */
  query: AqlQuery;
  /** Runtime Planner 使用的完整目标作用域与后端约束。 */
  target: AutomationTarget;
  /** 写回节点的 AQL 源码。 */
  onChange: (query: AqlQuery) => void;
}>;

type ScrollPosition = Readonly<{ left: number; top: number }>;

/** 基于 textarea 输入层与 Rust WASM 语言服务的自研 AQL Editor。 */
export function AqlEditor({ query, target, onChange }: AqlEditorProps) {
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const historyRef = useRef(new EditorHistory());
  const composingRef = useRef(false);
  const plannerState = useAqlInspection(target);
  const languageState = useLanguageDocument(query.source);
  const [selection, setSelection] = useState<EditorSelection>({ anchor: 0, head: 0 });
  const [scroll, setScroll] = useState<ScrollPosition>({ left: 0, top: 0 });
  const [completionItems, setCompletionItems] = useState<readonly CompletionItem[]>([]);
  const [activeHover, setActiveHover] = useState<Hover | null>(null);
  const [activeBrackets, setActiveBrackets] = useState<readonly EditorRange[]>([]);

  const languageDocument = languageState.phase === 'ready' ? languageState.document : null;
  const semanticTokens = languageDocument?.parsed.semantic_tokens ?? [];
  const diagnostics = useMemo(
    () => resolveDiagnostics(languageDocument?.parsed.diagnostics ?? null, plannerState),
    [languageDocument, plannerState],
  );
  const lineCount = useMemo(
    () => new LineIndex(query.source).lineCount,
    [query.source],
  );
  const formattedSource = languageDocument?.formatted_source ?? null;
  const codeActions = languageState.phase === 'ready'
    ? languageState.service.codeActions(query.source)
    : [];

  /** 写回文档并在下一次 DOM 更新后恢复精确 selection。 */
  const commit = (
    source: string,
    nextSelection: EditorSelection,
    recordHistory: boolean,
  ) => {
    if (recordHistory) {
      historyRef.current.push({ text: query.source, selection });
    }
    setSelection(nextSelection);
    onChange({ ...query, source });
    if (!composingRef.current) {
      queueMicrotask(() => {
        inputRef.current?.setSelectionRange(nextSelection.anchor, nextSelection.head);
      });
    }
  };

  /** 执行与 React 无关的编辑命令。 */
  const runCommand = (
    command: (text: string, currentSelection: EditorSelection) => {
      text: string;
      selection: EditorSelection;
    },
  ) => {
    const result = command(query.source, selection);
    commit(result.text, result.selection, true);
  };

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    const nextSelection = readSelection(event.currentTarget);
    commit(event.currentTarget.value, nextSelection, !composingRef.current);
  };

  const handleSelect = () => {
    const input = inputRef.current;
    if (!input) {
      return;
    }
    const nextSelection = readSelection(input);
    setSelection(nextSelection);
    if (languageState.phase === 'ready') {
      const position = new LineIndex(query.source).toPosition(nextSelection.head);
      setActiveHover(languageState.service.hover(query.source, position));
      setActiveBrackets(languageState.service.bracketPair(query.source, position) ?? []);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (composingRef.current) {
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.code === 'Space') {
      event.preventDefault();
      if (languageState.phase === 'ready') {
        const position = new LineIndex(query.source).toPosition(selection.head);
        setCompletionItems(languageState.service.completions(query.source, position));
      }
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      const snapshot = event.shiftKey
        ? historyRef.current.redo({ text: query.source, selection })
        : historyRef.current.undo({ text: query.source, selection });
      if (snapshot) {
        commit(snapshot.text, snapshot.selection, false);
      }
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') {
      event.preventDefault();
      const snapshot = historyRef.current.redo({ text: query.source, selection });
      if (snapshot) {
        commit(snapshot.text, snapshot.selection, false);
      }
      return;
    }
    if (event.key === '(') {
      event.preventDefault();
      runCommand(insertParenthesisPair);
    } else if (event.key === ')') {
      event.preventDefault();
      runCommand(skipClosingParenthesis);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      runCommand(insertIndentedLine);
    } else if (event.key === 'Tab') {
      event.preventDefault();
      runCommand(indentSelection);
    } else if (event.key === 'Escape') {
      setCompletionItems([]);
      setActiveHover(null);
    }
  };

  const applyCompletion = (item: CompletionItem) => {
    const edit = toOffsetEdit(query.source, {
      range: item.replacement_range,
      new_text: item.insert_text,
    });
    const nextSource = new AqlDocument(query.source)
      .replace(edit.start, edit.end, edit.newText)
      .text;
    const caretOffset = edit.start + edit.newText.length - (edit.newText.endsWith('()') ? 1 : 0);
    commit(nextSource, { anchor: caretOffset, head: caretOffset }, true);
    setCompletionItems([]);
  };

  const handleScroll = (event: UIEvent<HTMLTextAreaElement>) => {
    setScroll({ left: event.currentTarget.scrollLeft, top: event.currentTarget.scrollTop });
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] font-semibold text-slate-700">
          查找规则 <span className="font-normal text-slate-400">AQL</span>
        </span>
        <div className="flex items-center gap-1">
          {codeActions.length > 0 ? (
            <button
              type="button"
              className="flex h-7 items-center gap-1 rounded-md border border-amber-200 bg-amber-50 px-2 text-[10px] font-medium text-amber-700 hover:bg-amber-100"
              onClick={() => commit(
                new AqlDocument(query.source).apply(codeActions).text,
                selection,
                true,
              )}
            >
              <Wrench className="size-3" aria-hidden="true" />
              快速修复
            </button>
          ) : null}
          <button
            type="button"
            className="flex h-7 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600 hover:border-blue-300 hover:text-blue-700 disabled:cursor-not-allowed disabled:opacity-40"
            disabled={!formattedSource}
            onClick={() => {
              if (formattedSource) {
                const end = formattedSource.length;
                commit(formattedSource, { anchor: end, head: end }, true);
              }
            }}
          >
            <WandSparkles className="size-3" aria-hidden="true" />
            格式化
          </button>
        </div>
      </div>
      <div
        className={
          'relative flex h-[164px] min-h-[132px] resize-y overflow-hidden rounded-md border bg-white focus-within:ring-2 ' +
          (diagnostics.some((diagnostic) => diagnostic.severity === 'error')
            ? 'border-rose-300 focus-within:border-rose-400 focus-within:ring-rose-100'
            : 'border-slate-300 focus-within:border-blue-400 focus-within:ring-blue-100')
        }
      >
        <Gutter lineCount={lineCount} diagnostics={diagnostics} scrollTop={scroll.top} />
        <div className="relative min-w-0 flex-1 overflow-hidden">
          {languageState.phase === 'ready' ? (
            <HighlightLayer
              source={query.source}
              tokens={semanticTokens}
              scrollLeft={scroll.left}
              scrollTop={scroll.top}
            />
          ) : null}
          <DecorationLayer
            source={query.source}
            diagnostics={diagnostics}
            scrollLeft={scroll.left}
            scrollTop={scroll.top}
          />
          <BracketLayer
            source={query.source}
            ranges={activeBrackets}
            scrollLeft={scroll.left}
            scrollTop={scroll.top}
          />
          <InputLayer
            inputRef={inputRef}
            source={query.source}
            invalid={diagnostics.some((diagnostic) => diagnostic.severity === 'error')}
            highlighted={languageState.phase === 'ready'}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onSelect={handleSelect}
            onScroll={handleScroll}
            onCompositionStart={() => {
              composingRef.current = true;
              historyRef.current.push({ text: query.source, selection });
            }}
            onCompositionEnd={() => {
              composingRef.current = false;
            }}
          />
          <CompletionPopup
            items={completionItems}
            onApply={applyCompletion}
            onClose={() => setCompletionItems([])}
          />
          <HoverPopup hover={activeHover} />
        </div>
      </div>
      <p className="text-[9px] leading-4 text-slate-400">
        Ctrl+Space 补全；<code className="font-mono text-slate-500">&gt;</code> 表示直接子元素，
        <code className="font-mono text-slate-500">&gt;&gt;</code> 表示任意后代。
      </p>
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
    </div>
  );
}

/** 从 textarea 读取有方向的 UTF-16 selection。 */
function readSelection(input: HTMLTextAreaElement): EditorSelection {
  return input.selectionDirection === 'backward'
    ? { anchor: input.selectionEnd, head: input.selectionStart }
    : { anchor: input.selectionStart, head: input.selectionEnd };
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
