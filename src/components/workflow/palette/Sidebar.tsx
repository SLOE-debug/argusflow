import { useState } from "react";
import { Blocks, Search, Workflow } from "lucide-react";
import type { EditorTab } from "../../../features/workflow";
import { Button, Input } from "../../ui";
import { DocumentList } from "./DocumentList";
import { NodeTree } from "./NodeTree";

/** 侧栏仅编排紧凑入口和搜索，流程与节点各自负责内容。 */
export function Sidebar({ tab }: { readonly tab?: EditorTab }) {
  const [mode, setMode] = useState<"documents" | "nodes">("nodes");
  const [queries, setQueries] = useState({ documents: "", nodes: "" });
  return (
    <aside className="flex h-full min-h-0 flex-col bg-panel">
      <div
        role="tablist"
        aria-label="侧栏视图"
        className="flex h-10 shrink-0 items-center gap-1.5 border-b border-line px-2"
        onKeyDown={(event) => {
          if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
          event.preventDefault();
          const next = mode === "nodes" ? "documents" : "nodes";
          setMode(next);
          event.currentTarget
            .querySelectorAll<HTMLButtonElement>('[role="tab"]')
            [next === "documents" ? 0 : 1]?.focus();
        }}
      >
        {(["documents", "nodes"] as const).map((item) => (
          <Button
            key={item}
            variant="ghost"
            role="tab"
            aria-label={item === "documents" ? "流程" : "节点"}
            title={item === "documents" ? "流程" : "节点"}
            aria-selected={mode === item}
            tabIndex={mode === item ? 0 : -1}
            className={
              "h-8 flex-1 rounded-lg border-0 " +
              (mode === item ? "bg-accent-soft text-accent" : "text-muted")
            }
            onClick={() => setMode(item)}
          >
            {item === "documents" ? (
              <Workflow size={17} />
            ) : (
              <Blocks size={17} />
            )}
          </Button>
        ))}
      </div>
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
