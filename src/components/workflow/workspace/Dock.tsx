import { useStore } from "zustand";
import { studio, type EditorTab } from "../../../features/workflow";
import { Button, ResizeHandle, Tabs } from "../../ui";
import { LogPanel, locate } from "../execution/LogPanel";
import { DataPanel } from "../data/DataPanel";
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
  if (!state.dockOpen) return null;
  const tabs: readonly {
    readonly id: typeof state.dock;
    readonly title: string;
  }[] = [
    { id: "logs", title: "日志" },
    { id: "problems", title: "问题" },
    { id: "data", title: "输入输出" },
  ];
  const navigation = (
    <Tabs
      label="运行信息"
      value={state.dock}
      onChange={(value) => studio.panel(value)}
      items={tabs.map((item) => ({
        value: item.id,
        title: item.title,
        label:
          item.title +
          (item.id === "problems" && state.problems.length
            ? ` (${state.problems.length})`
            : ""),
      }))}
    />
  );
  return (
    <section
      className="flex shrink-0 flex-col border-t border-line bg-surface"
      style={{ height }}
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
      {state.dock !== "logs" && (
        <div className="flex h-8 shrink-0 items-center gap-2 border-b border-line px-3">
          {navigation}
          <span className="flex-1" />
        </div>
      )}
      {state.dockOpen && (
        <div className="min-h-0 flex-1">
          {state.dock === "logs" && (
            <LogPanel run={state.run} navigation={navigation} />
          )}
          {state.dock === "data" && <DataPanel tab={tab} />}
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
