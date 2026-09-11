import { useStore } from "zustand";
import { ChevronDown, ChevronUp, Maximize2, Minimize2 } from "lucide-react";
import { studio, type EditorTab } from "../../../features/workflow";
import { Button, ResizeHandle } from "../../ui";
import { LogPanel, locate } from "../execution/LogPanel";
import { DataPanel } from "../data/DataPanel";
import { AqlDock } from "./AqlDock";
import { useState } from "react";
export function Dock({
  tab,
  height,
  onHeight,
}: {
  readonly tab: EditorTab;
  readonly height: number;
  readonly onHeight: (height: number) => void;
}) {
  const state = useStore(studio.store);
  const [maximized, setMaximized] = useState(false);
  const tabs: readonly {
    readonly id: typeof state.dock;
    readonly title: string;
  }[] = [
    { id: "logs", title: "日志" },
    { id: "problems", title: "问题" },
    { id: "data", title: "输入输出" },
    ...(state.aqlNode ? [{ id: "aql" as const, title: "目标查询" }] : []),
  ];
  return (
    <section
      className="flex shrink-0 flex-col border-t border-line bg-surface"
      style={{ height: state.dockOpen ? (maximized ? "65vh" : height) : 34 }}
    >
      {state.dockOpen && (
        <ResizeHandle
          axis="y"
          value={height}
          min={140}
          max={600}
          reverse
          onChange={onHeight}
        />
      )}
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-line px-3">
        {tabs.map((item) => (
          <Button
            key={item.id}
            variant="ghost"
            className={
              "h-8 rounded-none border-b-2 px-2 text-[11px] " +
              (state.dock === item.id
                ? "border-b-accent text-accent"
                : "border-b-transparent text-muted")
            }
            onClick={() =>
              studio.panel(
                item.id,
                item.id === "aql" ? (state.aqlNode ?? undefined) : undefined,
              )
            }
          >
            {item.title}
            {item.id === "problems" && state.problems.length > 0 && (
              <span className="rounded-full bg-danger-soft px-1.5 text-[10px] text-danger">
                {state.problems.length}
              </span>
            )}
          </Button>
        ))}
        <span className="flex-1" />
        {state.dockOpen && (
          <Button
            variant="ghost"
            aria-label="最大化编辑区"
            className="h-6 px-1"
            onClick={() => setMaximized(!maximized)}
          >
            {maximized ? <Minimize2 size={12} /> : <Maximize2 size={12} />}
          </Button>
        )}
        <Button
          variant="ghost"
          aria-label={state.dockOpen ? "收起底部面板" : "展开底部面板"}
          className="h-6 px-1"
          onClick={() => studio.toggleDock()}
        >
          {state.dockOpen ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
        </Button>
      </div>
      {state.dockOpen && (
        <div className="min-h-0 flex-1">
          {state.dock === "logs" && <LogPanel run={state.run} />}
          {state.dock === "data" && <DataPanel tab={tab} />}
          {state.dock === "aql" && state.aqlNode && (
            <AqlDock
              key={tab.file.id + state.aqlNode}
              tab={tab}
              nodeId={state.aqlNode}
            />
          )}
          {state.dock === "problems" && (
            <div className="h-full overflow-auto p-3">
              {!state.problems.length && (
                <p className="text-xs text-muted">没有发现问题。</p>
              )}
              {state.problems.map((problem, index) => (
                <Button
                  key={index}
                  variant="ghost"
                  className="h-auto min-h-8 w-full justify-start whitespace-normal text-left font-normal text-danger"
                  onClick={() => {
                    if (problem.scope)
                      void studio.safely(() =>
                        locate({
                          workflow: problem.workflow ?? tab.file.id,
                          scope: problem.scope!,
                          node: problem.node,
                          instance: "",
                          execution: null,
                        }),
                      );
                    else if (problem.node) studio.select([problem.node]);
                  }}
                >
                  {problem.message}
                </Button>
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
