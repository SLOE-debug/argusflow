import { useState } from "react";
import { Blocks, Search, Workflow } from "lucide-react";
import type { EditorTab } from "../../../features/workflow";
import { Tabs, Input } from "../../ui";
import { DocumentList } from "./DocumentList";
import { NodeTree } from "./NodeTree";

/** 侧栏仅编排紧凑入口和搜索，流程与节点各自负责内容。 */
export function Sidebar({ tab }: { readonly tab?: EditorTab }) {
  const [mode, setMode] = useState<"documents" | "nodes">("nodes");
  const [queries, setQueries] = useState({ documents: "", nodes: "" });
  return (
    <aside className="flex h-full min-h-0 flex-col bg-panel">
      <Tabs
        label="侧栏视图"
        value={mode}
        onChange={setMode}
        className="h-9 shrink-0 border-b border-line px-2"
        items={[
          {
            value: "documents" as const,
            title: "流程",
            label: <Workflow size={16} />,
          },
          {
            value: "nodes" as const,
            title: "节点",
            label: <Blocks size={16} />,
          },
        ]}
      />
      <div className="shrink-0 p-2">
        <Input
          controlSize="compact"
          leading={<Search size={13} />}
          aria-label={mode === "nodes" ? "搜索节点库" : "搜索工作流"}
          className="w-full"
          placeholder={mode === "nodes" ? "搜索节点…" : "搜索工作流…"}
          value={queries[mode]}
          onChange={(event) =>
            setQueries({ ...queries, [mode]: event.target.value })
          }
        />
      </div>
      <div
        hidden={mode !== "nodes"}
        className={mode === "nodes" ? "flex min-h-0 flex-1 flex-col" : "hidden"}
      >
        <NodeTree tab={tab} query={queries.nodes} />
      </div>
      {mode === "documents" && <DocumentList query={queries.documents} />}
    </aside>
  );
}
