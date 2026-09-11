import { useCallback, useMemo, useRef, useState } from "react";
import { useStore } from "zustand";
import { ChevronRight, Plus } from "lucide-react";
import { compose, inverse, screenToWorld, type FlowPoint } from "../../../flow";
import {
  buildScene,
  childScopes,
  nodeById,
  studio,
  type EditorTab,
} from "../../../features/workflow";
import { Button } from "../../ui";
import { CanvasMenu } from "./CanvasMenu";
import { NodeSearch } from "../palette/NodeSearch";
import { ScopeGraph } from "./ScopeGraph";
import { useCanvasNavigation } from "./useCanvasNavigation";
import { useCanvasCommands } from "./useCanvasCommands";
import { useCanvasPointer, type CanvasToolMode } from "./useCanvasPointer";
import { CanvasTools } from "./CanvasTools";
import { useElementSize } from "./useElementSize";
import { nodeStates } from "../execution/nodeStates";

/** 画布入口只编排渲染层和单职责手势模块。 */
export function Canvas({ tab }: { readonly tab: EditorTab }) {
  const host = useRef<HTMLDivElement>(null);
  const size = useElementSize(host);
  const run = useStore(studio.store, (state) => state.run);
  const states = useMemo(
    () => nodeStates(run, tab.file.id),
    [run, tab.file.id],
  );
  const scene = useMemo(() => buildScene(tab.file), [tab.file]);
  const active =
    scene.scopes[tab.scope] ?? scene.scopes[tab.file.definition.root];
  const rootView = compose(tab.viewport, inverse(active.transform));
  const [search, setSearch] = useState<{
    readonly point: FlowPoint;
    readonly source?: string | null;
  } | null>(null);
  const [menu, setMenu] = useState<{
    readonly x: number;
    readonly y: number;
    readonly point: FlowPoint;
    readonly edge?: string;
  } | null>(null);
  const [minimap, setMinimap] = useState(false);
  const [toolMode, setToolMode] = useState<CanvasToolMode>("select");
  const addConnected = useCallback(
    (point: FlowPoint, source: string) => setSearch({ point, source }),
    [],
  );
  const gestures = useCanvasPointer(host, scene, addConnected, toolMode);
  useCanvasNavigation(host, scene);
  const activate = useCallback(
    (scope: string) => {
      const target = scene.scopes[scope];
      if (target) studio.view(compose(rootView, target.transform), scope);
    },
    [scene, rootView],
  );
  const cancel = useCallback(() => {
    gestures.cancel();
    setMenu(null);
    setSearch(null);
  }, [gestures]);
  const add = useCallback(
    () =>
      setSearch({
        point:
          gestures.pointer.current ??
          screenToWorld(
            { x: size.width / 2, y: size.height / 2 },
            tab.viewport,
          ),
      }),
    [gestures.pointer, size, tab.viewport],
  );
  useCanvasCommands(host, gestures.pointer, add, activate, cancel);
  const closeMenu = useCallback(() => {
    setMenu(null);
    host.current?.focus();
  }, []);
  const crumbs = [];
  let current: typeof active | undefined = active;
  while (current) {
    crumbs.unshift(current);
    current = current.parent ? scene.scopes[current.parent] : undefined;
  }
  return (
    <section className="relative flex min-h-0 min-w-0 flex-1 flex-col bg-canvas">
      {active.parent && (
        <div className="absolute left-4 top-3 z-20 flex max-w-[calc(100%-220px)] items-center gap-1 rounded-md bg-surface/90 px-2 py-1 shadow-sm">
          {crumbs.map((scope, index) => (
            <span key={scope.id} className="flex min-w-0 items-center gap-1">
              {index > 0 && <ChevronRight size={12} className="text-muted" />}
              <Button
                variant="ghost"
                className="h-6 max-w-44 truncate px-1 text-[11px]"
                onClick={() => activate(scope.id)}
              >
                {scope.label}
              </Button>
            </span>
          ))}
        </div>
      )}
      <div
        ref={host}
        tabIndex={0}
        role="application"
        aria-label="工作流画布"
        className={
          "relative min-h-0 min-w-0 flex-1 overflow-hidden outline-none " +
          (toolMode === "pan" ? "cursor-grab active:cursor-grabbing" : "")
        }
        style={{
          backgroundImage:
            "radial-gradient(var(--af-grid) 1px, transparent 1px)",
          backgroundSize:
            20 * tab.viewport.zoom + "px " + 20 * tab.viewport.zoom + "px",
          backgroundPosition: tab.viewport.x + "px " + tab.viewport.y + "px",
        }}
        onPointerDown={gestures.down}
        onPointerMove={gestures.move}
        onPointerUp={gestures.up}
        onPointerCancel={gestures.cancel}
        onKeyDown={(event) => {
          if (event.code === "Space" && event.target === event.currentTarget) {
            event.preventDefault();
            gestures.space.current = true;
          }
        }}
        onKeyUp={(event) => {
          if (event.code === "Space") gestures.space.current = false;
        }}
        onBlur={() => {
          gestures.space.current = false;
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          const rect = event.currentTarget.getBoundingClientRect();
          const element = event.target as Element;
          const node = element.closest<HTMLElement>("[data-node]");
          if (
            node?.dataset.nodeScope === tab.scope &&
            !tab.selected.includes(node.dataset.node!)
          )
            studio.select([node.dataset.node!]);
          setMenu({
            x: event.clientX,
            y: event.clientY,
            point: screenToWorld(
              { x: event.clientX - rect.left, y: event.clientY - rect.top },
              tab.viewport,
            ),
            edge: element.closest<HTMLElement>("[data-edge-source]")?.dataset
              .edgeSource,
          });
        }}
        onDoubleClick={(event) => {
          if (toolMode === "pan") return;
          const target = event.target as Element;
          const element = target.closest<HTMLElement>("[data-node]");
          if (element) {
            const node = nodeById(tab.file, element.dataset.node!);
            if (!node) return;
            const child = childScopes(node.action)[0];
            if (child) activate(child.id);
            else if (node.action.kind === "call_workflow")
              void studio.safely(
                () =>
                  node.action.kind === "call_workflow" &&
                  studio.open(node.action.workflow),
              );
            else {
              studio.select([node.id]);
              document
                .querySelector<HTMLInputElement>("[data-node-title]")
                ?.focus();
            }
          } else if (!studio.readonly) {
            const rect = event.currentTarget.getBoundingClientRect();
            setSearch({
              point: screenToWorld(
                { x: event.clientX - rect.left, y: event.clientY - rect.top },
                tab.viewport,
              ),
              source:
                target.closest<HTMLElement>("[data-edge-source]")?.dataset
                  .edgeSource,
            });
          }
        }}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes("application/argusflow-node")) {
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
          }
        }}
        onDrop={(event) => {
          event.preventDefault();
          const kind = event.dataTransfer.getData("application/argusflow-node");
          const rect = event.currentTarget.getBoundingClientRect();
          if (kind)
            void studio.safely(() =>
              studio.add(
                kind,
                screenToWorld(
                  { x: event.clientX - rect.left, y: event.clientY - rect.top },
                  tab.viewport,
                ),
              ),
            );
        }}
      >
        <div
          className="absolute origin-top-left"
          style={{
            transform:
              "translate(" +
              rootView.x +
              "px," +
              rootView.y +
              "px) scale(" +
              rootView.zoom +
              ")",
          }}
        >
          <ScopeGraph
            file={tab.file}
            scene={scene}
            scopeId={tab.file.definition.root}
            activeScope={active.id}
            selected={tab.selected}
            rootView={rootView}
            width={size.width}
            height={size.height}
            run={run}
            states={states}
          />
        </div>
        {gestures.box && (
          <div
            className="pointer-events-none absolute border border-accent bg-accent/10"
            style={{
              left: gestures.box.x * tab.viewport.zoom + tab.viewport.x,
              top: gestures.box.y * tab.viewport.zoom + tab.viewport.y,
              width: gestures.box.width * tab.viewport.zoom,
              height: gestures.box.height * tab.viewport.zoom,
            }}
          />
        )}
        {gestures.wire && (
          <svg className="pointer-events-none absolute inset-0 size-full">
            <path
              d={
                "M " +
                gestures.wire.from.x +
                " " +
                gestures.wire.from.y +
                " L " +
                gestures.wire.to.x +
                " " +
                gestures.wire.to.y
              }
              stroke="var(--af-accent)"
              strokeWidth={2}
              fill="none"
            />
          </svg>
        )}
        {!active.nodes.length && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <div className="mb-3 text-sm font-medium">从一个步骤开始</div>
              <Button variant="primary" onClick={add}>
                <Plus size={14} />
                添加第一个节点
              </Button>
              <p className="mt-3 text-xs text-muted">
                也可拖入节点，或按 Tab 搜索
              </p>
            </div>
          </div>
        )}
      </div>
      <CanvasTools
        tab={tab}
        scope={active}
        size={size}
        mode={toolMode}
        onModeChange={(mode) => {
          gestures.cancel();
          setToolMode(mode);
        }}
        minimap={minimap}
        onMinimapChange={() => setMinimap(!minimap)}
      />
      {minimap && (
        <svg
          className="absolute bottom-4 right-4 z-20 h-24 w-40 rounded-lg border border-line bg-surface/95 shadow-sm"
          preserveAspectRatio="none"
          viewBox={[
            active.bounds.x,
            active.bounds.y,
            active.bounds.width,
            active.bounds.height,
          ].join(" ")}
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            const x =
              active.bounds.x +
              ((event.clientX - rect.left) / rect.width) * active.bounds.width;
            const y =
              active.bounds.y +
              ((event.clientY - rect.top) / rect.height) * active.bounds.height;
            studio.view({
              ...tab.viewport,
              x: size.width / 2 - x * tab.viewport.zoom,
              y: size.height / 2 - y * tab.viewport.zoom,
            });
          }}
        >
          {active.nodes.map((node) => (
            <rect
              key={node.id}
              x={node.x}
              y={node.y}
              width={node.width}
              height={node.height}
              rx={6}
              fill={
                tab.selected.includes(node.id)
                  ? "var(--af-accent)"
                  : "var(--af-strong)"
              }
            />
          ))}
        </svg>
      )}
      {search && (
        <NodeSearch
          onClose={() => {
            setSearch(null);
            host.current?.focus();
          }}
          onPick={(kind) => {
            void studio.safely(() =>
              studio.add(kind, search.point, search.source),
            );
            setSearch(null);
            host.current?.focus();
          }}
        />
      )}
      {menu && (
        <CanvasMenu
          tab={tab}
          menu={menu}
          onClose={closeMenu}
          onAdd={() => setSearch({ point: menu.point, source: menu.edge })}
        />
      )}
    </section>
  );
}
