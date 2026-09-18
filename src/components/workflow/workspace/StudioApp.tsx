import { useEffect, useState } from "react";
import { RecorderBar } from "../../recorder";
import { useStore } from "zustand";
import { initializeDesktop } from "./lifecycle";
import { studio } from "../../../features/workflow";
import { ResizeHandle } from "../../ui";
import { TitleBar } from "../../shell/TitleBar";
import { Sidebar } from "../palette/Sidebar";
import { Canvas } from "../canvas/Canvas";
import { Inspector } from "../inspector/Inspector";
import { Toolbar } from "./Toolbar";
import { StatusBar } from "./StatusBar";
import { Welcome } from "./Welcome";
import { WorkflowTabs } from "./WorkflowTabs";
import { Dock } from "./Dock";
import { StudioToast } from "./StudioToast";
import { RunInputsDialog } from "../data/RunInputsDialog";
import { usePanelSize, usePanelVisible } from "./preferences";

/** 工作台根组件只负责面板与会话装配。 */
export function StudioApp() {
  const state = useStore(studio.store);
  const tab = state.active ? state.tabs[state.active] : undefined;
  const [left, setLeft] = usePanelVisible("left"),
    [right, setRight] = usePanelVisible("right");
  const [leftWidth, setLeftWidth] = usePanelSize("left", 230);
  const [rightWidth, setRightWidth] = usePanelSize("right", 310);
  const [dockHeight, setDockHeight] = usePanelSize("dock", 230);
  const [runDialog, setRunDialog] = useState(false);
  useEffect(() => {
    void studio.safely(initializeDesktop);
  }, []);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void studio.safely(() => studio.flushAll());
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  return (
    <main className="flex h-screen min-h-[600px] min-w-[1000px] flex-col overflow-hidden bg-app font-sans text-ink">
      <TitleBar menu={<RecorderBar />} tabs={<WorkflowTabs />} />
      <StudioToast />
      <div className="flex min-h-0 flex-1">
        {left && (
          <>
            <div
              className="shrink-0 border-r border-line"
              style={{ width: leftWidth }}
            >
              <Sidebar tab={tab} />
            </div>
            <ResizeHandle
              axis="x"
              value={leftWidth}
              min={180}
              max={340}
              onChange={setLeftWidth}
            />
          </>
        )}
        {tab ? (
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <div className="flex min-h-0 flex-1">
              <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                <Toolbar
                  tab={tab}
                  onRun={() => {
                    if (
                      Object.keys(tab.file.definition.inputs).length ||
                      Object.keys(tab.file.definition.resources).length
                    )
                      setRunDialog(true);
                    else void studio.safely(() => studio.run({}));
                  }}
                />
                <Canvas key={tab.file.id} tab={tab} />
              </div>
              {right && (
                <>
                  <ResizeHandle
                    axis="x"
                    value={rightWidth}
                    min={260}
                    max={720}
                    reverse
                    onChange={setRightWidth}
                  />
                  <div
                    className="shrink-0 border-l border-line"
                    style={{ width: rightWidth }}
                  >
                    <Inspector
                      key={tab.file.id + tab.selected.join(",")}
                      tab={tab}
                      onClose={() => setRight(false)}
                    />
                  </div>
                </>
              )}
            </div>
            <Dock tab={tab} height={dockHeight} onHeight={setDockHeight} />
          </div>
        ) : (
          <Welcome />
        )}
      </div>
      <StatusBar
        tab={tab}
        left={left}
        right={right}
        dockOpen={state.dockOpen}
        onLeft={() => setLeft(!left)}
        onRight={() => setRight(!right)}
      />
      {runDialog && tab && (
        <RunInputsDialog tab={tab} onClose={() => setRunDialog(false)} />
      )}
    </main>
  );
}
