import { useState } from "react";
import { Plus } from "lucide-react";
import {
  availableSymbols,
  nextPortName,
  scopeCanComplete,
  setWorkflowResult,
  studio,
  updateWorkflowPort,
  type EditorTab,
} from "../../../features/workflow";
import { Button, Select } from "../../ui";
import { useDocuments } from "../inspector/useDocuments";
import { ResultRow } from "./ResultRow";

/** 先选择需要展示的结果，再自动创建名称与完整类型。 */
export function WorkflowResults({ tab }: { readonly tab: EditorTab }) {
  const documents = useDocuments();
  const [adding, setAdding] = useState(false);
  const definition = tab.file.definition;
  const symbols = availableSymbols(
    tab.file,
    definition.root,
    undefined,
    documents,
  ).values;
  const canComplete = scopeCanComplete(tab.file, definition.root);
  const addManual = () => {
    studio.edit((file) =>
      updateWorkflowPort(
        file,
        "outputs",
        nextPortName(file.definition.outputs, "结果"),
        { type: "text" },
      ),
    );
    setAdding(false);
  };
  return (
    <section className="min-w-0 p-4">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold">结果设置</h3>
        <Button
          variant="ghost"
          onClick={() => (canComplete ? setAdding(true) : addManual())}
        >
          <Plus size={14} />
          添加结果
        </Button>
      </div>
      <p className="mb-4 text-sm leading-6 text-muted">
        {canComplete
          ? "选择要保留的内容。运行后，在「运行结果」中查看。"
          : "请先将步骤连接到「结束」，或到「返回结果」步骤中选择要显示的内容。"}
      </p>
      {adding && (
        <div className="mb-3 space-y-3 rounded-lg border border-line bg-subtle/40 p-3">
          {symbols.length ? (
            <Select
              aria-label="选择要显示的内容"
              className="w-full"
              value=""
              placeholder="选择要显示的内容"
              options={symbols.map((item) => ({
                value: JSON.stringify(item.expression),
                label: item.label,
              }))}
              onValueChange={(selected) => {
                const source = symbols.find(
                  (item) => JSON.stringify(item.expression) === selected,
                );
                if (!source) return;
                studio.edit((file) =>
                  setWorkflowResult(
                    file,
                    nextPortName(
                      file.definition.outputs,
                      source.label.split(" · ").at(-1) ?? "结果",
                    ),
                    source.type,
                    source.expression,
                  ),
                );
                setAdding(false);
              }}
            />
          ) : (
            <p className="text-sm text-muted">
              还没有可选择的内容。请先添加并连接能产生结果的步骤，也可以手动填写。
            </p>
          )}
          <div className="flex gap-2">
            <Button variant="ghost" onClick={addManual}>
              手动填写内容
            </Button>
            <Button variant="ghost" onClick={() => setAdding(false)}>
              取消
            </Button>
          </div>
        </div>
      )}
      <div className="space-y-3">
        {Object.entries(definition.outputs).map(([name, type]) => (
          <ResultRow
            key={name}
            tab={tab}
            name={name}
            type={type}
            symbols={symbols}
            canComplete={canComplete}
          />
        ))}
        {!Object.keys(definition.outputs).length && !adding && (
          <p className="text-sm text-muted">
            点击「添加结果」，选择结束后需要查看的内容。
          </p>
        )}
      </div>
    </section>
  );
}
