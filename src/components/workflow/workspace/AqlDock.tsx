import {
  availableSymbols,
  defaultValue,
  literal,
  nodeById,
  studio,
  updateNode,
  useQueryEditor,
  type EditorTab,
} from "../../../features/workflow";
import { CodeEditor } from "../../editor/monaco/CodeEditor";
import { Button, FormField } from "../../ui";
import { ValueField } from "../value-editor/ValueField";

/** 节点选择切换时由 key 重建，旧异步分析不能提交到新节点。 */
export function AqlDock({
  tab,
  nodeId,
}: {
  readonly tab: EditorTab;
  readonly nodeId: string;
}) {
  const node = nodeById(tab.file, nodeId);
  const {
    service,
    error,
    setError,
    source,
    parameters,
    analysis,
    applying,
    change,
    apply,
    setComposing,
  } = useQueryEditor(tab, nodeId);
  const task = node?.action.kind === "task" ? node.action.task : null;
  const symbols = availableSymbols(tab.file, tab.scope, nodeId);
  return (
    <div className="flex h-full min-h-0">
      <div className="flex min-w-0 flex-1 flex-col border-r border-line">
        <div className="flex h-9 shrink-0 items-center justify-between border-b border-line px-3">
          <span className="text-[11px] text-muted">
            中文 AQL ·{" "}
            {analysis?.diagnostics.length
              ? analysis.diagnostics.length + " 处问题"
              : "实时语法检查"}
          </span>
          <Button
            className="h-6 text-[11px]"
            disabled={!analysis?.english || studio.readonly || applying}
            onClick={() => {
              void apply();
            }}
          >
            应用查询
          </Button>
        </div>
        {service ? (
          <CodeEditor
            source={source}
            service={service}
            diagnostics={analysis?.diagnostics ?? []}
            onChange={change}
            onComposition={setComposing}
            onError={() => setError("查询编辑器加载失败")}
            compact
            readOnly={studio.readonly}
          />
        ) : (
          <p className="p-4 text-xs text-muted">正在加载查询编辑器…</p>
        )}
      </div>
      <fieldset
        disabled={studio.readonly}
        className="w-72 shrink-0 overflow-auto p-3"
      >
        <div className="mb-3 text-xs font-medium">查询参数</div>
        {!Object.keys(parameters).length && (
          <p className="text-[11px] leading-5 text-muted">
            查询参数通过检查后会显示在这里。
          </p>
        )}
        {Object.entries(parameters).map(([key, type]) => (
          <FormField key={key} label={key}>
            <ValueField
              value={task?.inputs[key] ?? literal(type, defaultValue(type))}
              type={type}
              symbols={symbols.values}
              pending={tab.file.editor.drafts[nodeId + ":input." + key]}
              onInvalid={(source) =>
                studio.draft(nodeId, "input." + key, source)
              }
              onChange={(value) =>
                studio.draft(nodeId, "input." + key, null, (file) =>
                  updateNode(file, nodeId, (current) =>
                    current.action.kind === "task"
                      ? {
                          ...current,
                          action: {
                            ...current.action,
                            task: {
                              ...current.action.task,
                              inputs: {
                                ...current.action.task.inputs,
                                [key]: value,
                              },
                            },
                          },
                        }
                      : current,
                  ),
                )
              }
            />
          </FormField>
        ))}
        {analysis?.diagnostics.map((item, index) => (
          <p key={index} className="mt-2 text-[11px] text-danger">
            {item.message}
          </p>
        ))}
        {error && (
          <p role="alert" className="mt-2 text-xs text-danger">
            {error}
          </p>
        )}
      </fieldset>
    </div>
  );
}
