import {
  availableSymbols,
  INT,
  integer,
  studio,
  updateNode,
  type EditorTab,
  type Expr,
  type WorkflowNode,
} from "../../../features/workflow";
import { FormField, Switch } from "../../ui";
import { ValueField } from "../value-editor/ValueField";
import { useDocuments } from "./useDocuments";

/** 所有节点共用同一时限表达式，不复制任务内部的超时字段。 */
export function TimeoutFields({
  node,
  tab,
}: {
  readonly node: WorkflowNode;
  readonly tab: EditorTab;
}) {
  const documents = useDocuments();
  const symbols = availableSymbols(tab.file, tab.scope, node.id, documents);
  const change = (value: Expr | null) =>
    studio.draft(node.id, "timeout", null, (file) =>
      updateNode(file, node.id, (current) => ({
        ...current,
        timeout_ms: value,
      })),
    );
  return (
    <section className="space-y-3 border-t border-line pt-3 mt-6">
      <Switch
        label="限制节点等待时间"
        checked={node.timeout_ms !== null}
        onCheckedChange={(enabled) => change(enabled ? integer("10000") : null)}
      />
      {node.timeout_ms && (
        <FormField label="最长等待（毫秒）">
          <ValueField
            value={node.timeout_ms}
            type={INT}
            symbols={symbols.values}
            pending={tab.file.editor.drafts[node.id + ":timeout"]}
            onChange={change}
            onInvalid={(source) => studio.draft(node.id, "timeout", source)}
          />
        </FormField>
      )}
      <p className="text-[11px] leading-5 text-muted">
        进入节点后开始计时。超时将终止整条流程，不会自动重试。
      </p>
    </section>
  );
}
