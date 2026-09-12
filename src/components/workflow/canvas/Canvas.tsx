import { useCallback, useId, useMemo, useRef, useState } from "react";
import { useStore } from "zustand";
import { ChevronRight, Plus } from "lucide-react";
import {
  buildCanvasScene,
  studio,
  type EditorTab,
} from "../../../features/workflow";
import { useTheme } from "../../../features/themes";
import { Button, IconButton } from "../../ui";
import { NodeSearch } from "../palette/NodeSearch";
import { nodeStates } from "../execution/nodeStates";
import { CanvasMenu } from "./CanvasMenu";
import { CanvasErrorBoundary } from "./CanvasErrorBoundary";
import { CanvasTools } from "./CanvasTools";
import { editingScope, presentNodes, type CanvasLocation } from "./scene";
import { useCanvasNavigation } from "./useCanvasNavigation";
import { useCanvasCommands } from "./useCanvasCommands";
import { useCanvasPointer, type CanvasToolMode } from "./useCanvasPointer";
import { useCanvasActions } from "./useCanvasActions";
import { useCanvasRenderer } from "./rendering/useCanvasRenderer";
import { InteractionPreview } from "./rendering/interactionPreview";
import { useElementSize } from "./useElementSize";

/** 画布入口只装配表面、交互模块与 DOM 浮层。 */
export function Canvas({ tab }: { readonly tab: EditorTab }) {
  return (
    <CanvasErrorBoundary>
      <CanvasSurface tab={tab} />
    </CanvasErrorBoundary>
  );
}

/** 单个工作流会话的表面与交互状态。 */
function CanvasSurface({ tab }: { readonly tab: EditorTab }) {
  const host = useRef<HTMLDivElement>(null);
  const surface = useRef<HTMLCanvasElement>(null);
  const size = useElementSize(host);
  const { colors } = useTheme();
  const run = useStore(studio.store, (state) => state.run);
  const states = useMemo(
    () => nodeStates(run, tab.file.id),
    [run, tab.file.id],
  );
  const scene = useMemo(() => buildCanvasScene(tab.file), [tab.file]);
  const nodes = useMemo(() => presentNodes(tab.file), [tab.file]);
  const active = editingScope(scene, tab.scope);
  const [mode, setMode] = useState<CanvasToolMode>("select");
  const preview = useMemo(() => new InteractionPreview(), []);
  const navigation = useCanvasNavigation(host, scene, size);
  /** 连线手势与浮层分别拥有状态，只通过明确落点回调连接。 */
  const connected = useRef<(location: CanvasLocation) => void>(() => {});
  const addConnected = useCallback(
    (location: CanvasLocation) => connected.current(location),
    [],
  );
  const gestures = useCanvasPointer(
    host,
    scene,
    nodes,
    addConnected,
    mode,
    navigation.cancel,
    preview,
  );
  const actions = useCanvasActions(
    host,
    scene,
    nodes,
    gestures.pointer,
    size,
    navigation.focus,
    mode === "pan",
  );
  connected.current = actions.setSearch;
  const cancel = () => {
    navigation.cancel();
    gestures.cancel();
    actions.close();
  };
  useCanvasCommands(
    host,
    actions.location,
    actions.add,
    navigation.focus,
    cancel,
  );
  const error = useCanvasRenderer(
    surface,
    {
      scene,
      nodes,
      camera: tab.viewport,
      width: size.width,
      height: size.height,
      colors,
      selected: tab.selected,
      selectedEdge: tab.selectedEdge,
      readonly: studio.readonly,
      scope: active.id,
      states,
      runningDocument: Boolean(run?.documents.includes(tab.file.id)),
      box: gestures.box,
    },
    preview,
  );
  const description = useId();
  const crumbs = [];
  let current: typeof active | undefined = active;
  while (current) {
    crumbs.unshift(current);
    current = current.parent ? scene.scopes[current.parent] : undefined;
  }
  const fit = () => navigation.focus(scene.root);
  const selection = tab.selected
    .map((id) => nodes.get(id)?.title)
    .filter(Boolean)
    .join("、");
  return (
    <section className="relative flex min-h-0 min-w-0 flex-1 flex-col bg-canvas">
      <div
        ref={host}
        tabIndex={0}
        role="application"
        aria-label="工作流画布"
        aria-describedby={description}
        className={
          "relative min-h-0 min-w-0 flex-1 overflow-hidden outline-none after:pointer-events-none after:absolute after:bottom-2 after:left-1/2 after:h-1 after:w-8 after:-translate-x-1/2 after:rounded-full after:bg-accent/30 after:opacity-0 focus-visible:after:opacity-100 " +
          (mode === "pan"
            ? "cursor-grab active:cursor-grabbing"
            : gestures.cursor)
        }
        onPointerDown={gestures.down}
        onPointerMove={gestures.move}
        onPointerLeave={gestures.leave}
        onPointerUp={gestures.up}
        onPointerCancel={gestures.cancel}
        onLostPointerCapture={gestures.cancel}
        onWheelCapture={gestures.cancel}
        onKeyDown={(event) => {
          if (event.key === "Alt" && gestures.snapModifierChanged(true))
            event.preventDefault();
          if (event.code === "Space" && event.target === event.currentTarget) {
            event.preventDefault();
            gestures.space.current = true;
          }
        }}
        onKeyUp={(event) => {
          if (event.key === "Alt" && gestures.snapModifierChanged(false))
            event.preventDefault();
          if (event.code === "Space") gestures.space.current = false;
        }}
        onBlur={() => {
          gestures.space.current = false;
        }}
        onClick={(event) => {
          if (!gestures.consumeClick()) actions.click(event);
        }}
        onContextMenu={actions.openMenu}
        onDoubleClick={actions.doubleClick}
        onDragOver={actions.dragOver}
        onDrop={actions.drop}
      >
        <canvas
          ref={surface}
          aria-hidden="true"
          className="absolute inset-0 size-full touch-none"
        />
        <span id={description} className="sr-only">
          滚轮缩放，空格或中键拖动平移。Tab 添加节点，Ctrl+A 选择当前流程，Enter
          聚焦子流程。拖动节点时自动对齐，按住 Alt 暂停吸附。
        </span>
        <span role="status" aria-live="polite" className="sr-only">
          {tab.selectedEdge
            ? "已选择连线，可拖动两端改接"
            : selection
              ? "已选择：" + selection
              : "当前流程：" + active.label}
        </span>
      </div>
      {active.parent && (
        <nav
          aria-label="流程层级"
          className="absolute left-4 top-3 z-20 flex max-w-[calc(100%-280px)] items-center overflow-hidden rounded-lg border border-line bg-surface/95 px-2 py-1 shadow-sm"
        >
          {crumbs.map((scope, index) => (
            <span key={scope.id} className="flex min-w-0 items-center gap-1">
              {index > 0 && (
                <ChevronRight size={12} className="shrink-0 text-muted" />
              )}
              <Button
                variant="ghost"
                className="h-7 max-w-32 truncate px-1 text-xs"
                onClick={() => navigation.focus(scope.id)}
              >
                {scope.label}
              </Button>
            </span>
          ))}
        </nav>
      )}
      <CanvasTools
        tab={tab}
        scene={scene}
        size={size}
        mode={mode}
        onModeChange={(next) => {
          gestures.cancel();
          setMode(next);
        }}
        onFit={fit}
      />
      <IconButton
        aria-label="添加节点"
        disabled={studio.readonly}
        className="absolute bottom-5 right-5 z-20 size-14 rounded-full border-0 bg-accent text-on-accent shadow-lg hover:bg-accent-hover"
        onClick={actions.add}
      >
        <Plus size={28} />
      </IconButton>
      {error && (
        <p
          role="alert"
          className="absolute left-1/2 top-1/2 -translate-x-1/2 rounded-lg bg-surface p-4 text-sm text-danger"
        >
          {error}
        </p>
      )}
      {actions.search && (
        <NodeSearch
          onClose={actions.close}
          onPick={(kind) => {
            const target = actions.search!;
            void studio.safely(() =>
              studio.add(kind, target.point, target.connection, target.scope),
            );
            actions.close();
          }}
        />
      )}
      {actions.menu && (
        <CanvasMenu
          tab={tab}
          menu={actions.menu}
          onClose={actions.closeMenu}
          onAdd={() =>
            actions.setSearch({
              scope: actions.menu!.scope,
              point: actions.menu!.point,
              connection: actions.menu!.edge
                ? { kind: "insert", edge: actions.menu!.edge }
                : undefined,
            })
          }
        />
      )}
    </section>
  );
}
