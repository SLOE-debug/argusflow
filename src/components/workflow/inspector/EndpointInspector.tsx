import { Trash2, X } from "lucide-react";
import { studio, type EditorTab } from "../../../features/workflow";
import { Button } from "../../ui";
import { NodeIcon } from "../presentation/NodeIcon";

/** 起止标记只提供布局操作，不展示业务节点的执行配置。 */
export function EndpointInspector({
  kind,
  tab,
  onClose,
}: {
  readonly kind: "start" | "end";
  readonly tab: EditorTab;
  readonly onClose: () => void;
}) {
  const position = tab.file.editor.nodes[tab.selected[0]];
  return (
    <aside className="h-full overflow-auto bg-surface p-5">
      <header className="mb-5 flex items-center gap-2.5">
        <span className="shrink-0">
          <NodeIcon kind={kind} size={20} />
        </span>
        <h2 className="flex-1 text-sm font-semibold">
          {kind === "start" ? "开始" : "结束"}
        </h2>
        <Button variant="ghost" aria-label="关闭属性面板" onClick={onClose}>
          <X size={14} />
        </Button>
      </header>
      <p className="text-xs leading-6 text-muted">
        每个流程可以添加一个开始和一个结束。拖动可调整位置。
      </p>
      <p className="my-4 text-xs tabular-nums text-muted">
        位置：{position.x}，{position.y}
      </p>
      <Button
        variant="danger"
        disabled={studio.readonly}
        onClick={() => studio.remove()}
      >
        <Trash2 size={14} />
        删除
      </Button>
    </aside>
  );
}
