import { useRef, useState, type KeyboardEvent } from "react";
import { useStore } from "zustand";
import { ChevronRight } from "lucide-react";
import {
  buildScene,
  commonNodes,
  nodeUsage,
  studio,
  type EditorTab,
} from "../../../features/workflow";
import { NodeIcon } from "../presentation/NodeIcon";
import { visibleTreeRows, type TreeCategory, type TreeRow } from "./treeModel";

/** 紧凑节点树：目录导航与添加动作共享鼠标和键盘入口。 */
export function NodeTree({
  tab,
  query,
}: {
  readonly tab?: EditorTab;
  readonly query: string;
}) {
  const usage = useStore(nodeUsage.store);
  const [expanded, setExpanded] = useState<ReadonlySet<TreeCategory>>(
    () => new Set(["常用"]),
  );
  const [focused, setFocused] = useState("常用");
  const root = useRef<HTMLDivElement>(null);
  const rows = visibleTreeRows(query, expanded, commonNodes(usage.entries));
  const focusKey = rows.some((row) => row.key === focused)
    ? focused
    : rows[0]?.key;
  const editable = Boolean(tab) && !studio.readonly;
  const toggle = (category: TreeCategory) => {
    if (query.trim()) return;
    const next = new Set(expanded);
    if (next.has(category)) next.delete(category);
    else next.add(category);
    setExpanded(next);
  };
  const focusRow = (key?: string) => {
    if (!key) return;
    setFocused(key);
    const elements =
      root.current?.querySelectorAll<HTMLElement>('[role="treeitem"]');
    const index = rows.findIndex((row) => row.key === key);
    elements?.[index]?.focus();
  };
  const add = (row: TreeRow) => {
    if (row.type !== "node" || !tab || !editable) return;
    void studio.safely(() => {
      const scope = buildScene(tab.file).scopes[tab.scope];
      const last = scope.nodes.at(-1);
      studio.add(
        row.node.id,
        last ? { x: last.x + last.width + 80, y: last.y } : { x: 80, y: 100 },
      );
    });
  };
  const keyDown = (event: KeyboardEvent, row: TreeRow, index: number) => {
    if (
      ![
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "Enter",
        " ",
      ].includes(event.key)
    )
      return;
    event.preventDefault();
    event.stopPropagation();
    switch (event.key) {
      case "ArrowUp":
        focusRow(rows[Math.max(0, index - 1)]?.key);
        break;
      case "ArrowDown":
        focusRow(rows[Math.min(rows.length - 1, index + 1)]?.key);
        break;
      case "Home":
        focusRow(rows[0]?.key);
        break;
      case "End":
        focusRow(rows.at(-1)?.key);
        break;
      case "ArrowLeft":
        if (row.type === "node") focusRow(row.category);
        else if (row.expanded) toggle(row.category);
        break;
      case "ArrowRight":
        if (row.type === "category") {
          if (!row.expanded) toggle(row.category);
          else if (rows[index + 1]?.type === "node")
            focusRow(rows[index + 1].key);
        }
        break;
      default:
        if (row.type === "category") toggle(row.category);
        else add(row);
    }
  };
  const groups = rows.filter((row) => row.type === "category");
  return (
    <>
      <div
        ref={root}
        role="tree"
        aria-label="节点库"
        className="min-h-0 flex-1 overflow-y-auto px-2 pb-2"
      >
        {rows.map((row, index) => (
          <div
            key={row.key}
            role="treeitem"
            aria-label={row.type === "category" ? row.category : row.node.title}
            aria-level={row.type === "category" ? 1 : 2}
            aria-posinset={
              row.type === "category"
                ? groups.findIndex((item) => item.key === row.key) + 1
                : row.position
            }
            aria-setsize={row.type === "category" ? groups.length : row.count}
            aria-expanded={row.type === "category" ? row.expanded : undefined}
            aria-disabled={row.type === "node" ? !editable : undefined}
            tabIndex={row.key === focusKey ? 0 : -1}
            title={row.type === "node" ? row.node.description : undefined}
            draggable={row.type === "node" && editable}
            onDragStart={(event) => {
              if (row.type !== "node" || !editable) {
                event.preventDefault();
                return;
              }
              event.dataTransfer.setData(
                "application/argusflow-node",
                row.node.id,
              );
              event.dataTransfer.effectAllowed = "copy";
            }}
            onFocus={() => setFocused(row.key)}
            onClick={() => {
              focusRow(row.key);
              if (row.type === "category") toggle(row.category);
              else add(row);
            }}
            onKeyDown={(event) => keyDown(event, row, index)}
            className={
              "flex select-none items-center rounded-sm text-xs outline-none hover:bg-hover focus-visible:bg-accent-soft focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-accent " +
              (row.type === "category"
                ? "h-6 cursor-default gap-1 px-1 font-medium text-ink"
                : "h-7 gap-2 pr-2 text-ink " +
                  (editable
                    ? "cursor-grab active:cursor-grabbing"
                    : "cursor-default"))
            }
          >
            {row.type === "category" ? (
              <>
                <ChevronRight
                  size={12}
                  className={
                    "shrink-0 text-muted transition-transform " +
                    (row.expanded ? "rotate-90" : "")
                  }
                />
                <span>{row.category}</span>
                <span className="ml-auto text-[10px] font-normal tabular-nums text-muted">
                  {row.count}
                </span>
              </>
            ) : (
              <>
                <span
                  aria-hidden="true"
                  className="ml-2.5 flex h-full w-3 shrink-0 items-center border-l border-line"
                >
                  <span className="w-2 border-t border-line" />
                </span>
                <span
                  className={
                    row.category === "逻辑控制"
                      ? "shrink-0 text-structure"
                      : "shrink-0 text-accent"
                  }
                >
                  <NodeIcon kind={row.node.id} size={14} />
                </span>
                <span className="truncate">{row.node.title}</span>
              </>
            )}
          </div>
        ))}
        {!rows.length && (
          <p className="px-2 py-4 text-xs text-muted">没有匹配的节点</p>
        )}
      </div>
      <div className="shrink-0 border-t border-line px-3 py-1.5 text-[10px] text-muted">
        {usage.error ?? "拖入或点击添加 · Tab 搜索"}
      </div>
    </>
  );
}
