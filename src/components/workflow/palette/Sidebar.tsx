import { useState } from "react";
import { Search } from "lucide-react";
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
        className="flex h-7 shrink-0 items-stretch gap-1 border-b border-line px-2"
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
            aria-selected={mode === item}
            tabIndex={mode === item ? 0 : -1}
            className={
              "h-7 flex-1 rounded-none border-x-0 border-t-0 text-[11px] " +
              (mode === item
                ? "border-b-2 border-accent text-ink"
                : "text-muted")
            }
            onClick={() => setMode(item)}
          >
            {item === "documents" ? "流程" : "节点"}
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
