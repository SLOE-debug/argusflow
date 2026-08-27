import { useCallback } from 'react';

import {
  configureRuntimeExpressionLanguage,
  RUNTIME_EXPRESSION_LANGUAGE_ID,
  setRuntimeExpressionSuggestions,
} from '../../features/workflow/runtimeExpressionLanguage';
import type { WorkflowCanvasNode } from '../../features/workflow/workflowModel';
import { MonacoEditor, type MonacoApi } from '../ui/monaco';

type ExpressionEditorProps = Readonly<{
  /** Monaco 模型的稳定 URI。 */
  modelUri: string;
  /** 当前受限 Rhai 表达式源码。 */
  source: string;
  /** 实时工作流节点，用于提供节点与输出补全。 */
  nodes: ReadonlyArray<WorkflowCanvasNode>;
  /** 最近一次 Runtime prepare 返回的编译错误。 */
  compileError: string | null;
  /** 实时写回所属 ValueExpr。 */
  onChange: (source: string) => void;
}>;

/** 编辑 Runtime Value Plane 的受限 Rhai 表达式并展示后端编译诊断。 */
export function ExpressionEditor({
  modelUri,
  source,
  nodes,
  compileError,
  onChange,
}: ExpressionEditorProps) {
  /** 在 Monaco 创建模型前刷新当前文档补全并安装一次语言 provider。 */
  const configureLanguage = useCallback((monaco: MonacoApi) => {
    setRuntimeExpressionSuggestions(modelUri, nodes);
    configureRuntimeExpressionLanguage(monaco);
  }, [modelUri, nodes]);

  return (
    <section className="flex h-full min-h-0 flex-col bg-white">
      <div className="flex min-h-9 shrink-0 items-center border-b border-slate-200 bg-slate-50/70 px-3 py-1.5">
        <p className="text-[10px] text-slate-500">
          可使用输入、变量和节点数据；结果仅在输出映射中可用。支持 str、json、get 函数。
        </p>
      </div>
      <div className="min-h-0 flex-1 p-2">
        <MonacoEditor
          ariaLabel="运行时表达式"
          value={source}
          language={RUNTIME_EXPRESSION_LANGUAGE_ID}
          modelUri={modelUri}
          className="h-full min-h-0"
          configure={configureLanguage}
          options={{
            bracketPairColorization: { enabled: true },
            folding: false,
            glyphMargin: false,
            guides: { bracketPairs: true },
            lineNumbers: 'on',
            lineNumbersMinChars: 3,
            quickSuggestions: { other: true, comments: false, strings: false },
            suggestOnTriggerCharacters: true,
            wordWrap: 'on',
          }}
          onChange={onChange}
        />
      </div>
      <div className="shrink-0 border-t border-slate-200 px-3 py-2">
        {compileError ? (
          <p
            className="rounded-md bg-rose-50 px-2.5 py-2 text-[10px] leading-4 text-rose-700"
            role="alert"
          >
            {compileError}
          </p>
        ) : (
          <p className="text-[10px] text-slate-500">
            保存后运行检查会编译表达式，错误会显示在这里。
          </p>
        )}
      </div>
    </section>
  );
}
