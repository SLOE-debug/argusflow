import { useEffect, useId, useMemo, useRef, useState } from "react";
import { CodeEditor } from "../../editor/monaco";
import type { SymbolValue } from "../../../features/workflow";
import { expressionLanguage } from "./expressionLanguage";
/** 编译器注入式代码字段：实时检查，只有有效 AST 才提交，错误源码单独保留。 */
export function FormulaEditor<T>({
  source,
  symbols,
  compile,
  onChange,
  onInvalid,
  label = "值表达式",
  readOnly = false,
}: {
  readonly source: string;
  readonly symbols: readonly SymbolValue[];
  readonly compile: (source: string) => T;
  readonly onChange: (value: T) => void;
  readonly onInvalid?: (source: string) => void;
  readonly label?: string;
  readonly readOnly?: boolean;
}) {
  const id = useId(),
    currentSymbols = useRef(symbols);
  currentSymbols.current = symbols;
  const language = useMemo(
    () =>
      expressionLanguage(
        "workflow-formula-" + id,
        () => currentSymbols.current,
      ),
    [id],
  );
  const [draft, setDraft] = useState(source),
    [loadingError, setLoadingError] = useState("");
  const previousSource = useRef(source);
  useEffect(() => {
    if (previousSource.current === source) return;
    previousSource.current = source;
    try {
      if (JSON.stringify(compile(source)) === JSON.stringify(compile(draft)))
        return;
    } catch {
      /* 未完成的外部草稿也必须响应撤销。 */
    }
    setDraft(source);
  }, [source, draft, compile]);
  const [composing, setComposing] = useState(false);
  let message = "";
  try {
    compile(draft);
  } catch (error) {
    message = error instanceof Error ? error.message : String(error);
  }
  const lines = draft.split("\n");
  const commit = (text: string) => {
    let value: T;
    try {
      value = compile(text);
    } catch {
      onInvalid?.(text);
      return;
    }
    onChange(value);
  };
  const change = (text: string) => {
    setDraft(text);
    if (readOnly) return;
    if (composing) {
      onInvalid?.(text);
      return;
    }
    commit(text);
  };
  return (
    <div className="min-w-0">
      <div className="flex h-28 min-w-0 rounded-md border border-line bg-surface">
        <CodeEditor
          compact
          label={label}
          language={language}
          source={draft}
          readOnly={readOnly}
          diagnostics={
            message && !composing
              ? [
                  {
                    code: "expression",
                    message,
                    range: {
                      start: { line: 0, column: 0 },
                      end: {
                        line: lines.length - 1,
                        column: Math.max(1, lines.at(-1)?.length ?? 1),
                      },
                    },
                  },
                ]
              : []
          }
          onChange={change}
          onComposition={(active) => {
            setComposing(active);
            if (!active && !readOnly) {
              commit(draft);
            }
          }}
          onError={() => setLoadingError("代码编辑器加载失败")}
        />
      </div>
      {(message || loadingError) && (
        <p role="alert" className="mt-1 text-xs text-danger">
          {loadingError || message}
        </p>
      )}
    </div>
  );
}
