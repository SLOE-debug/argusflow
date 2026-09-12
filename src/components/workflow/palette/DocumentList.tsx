import { useStore } from "zustand";
import { useCallback, useState } from "react";
import { Plus, RefreshCw, FolderOpen, Pencil, Trash2 } from "lucide-react";
import {
  studio,
  documentReadonly,
  listedDocuments,
} from "../../../features/workflow";
import { IconButton, Menu } from "../../ui";
import { DocumentRow } from "./DocumentRow";
import { DocumentDialog, type DocumentAction } from "./DocumentDialog";

/** 工作流列表只展示固定数据目录的内容，不提供目录选择。 */
export function DocumentList({ query }: { readonly query: string }) {
  const state = useStore(studio.store);
  const ready = state.initialization.status === "ready";
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(
    null,
  );
  const [action, setAction] = useState<DocumentAction | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const all = listedDocuments(state);
  const current = menu ? all.find((item) => item.id === menu.id) : undefined;
  const documents = all.filter((item) =>
    item.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  return (
    <>
      <div className="flex h-7 shrink-0 items-center gap-1 px-2">
        <span className="mr-auto text-[11px] text-muted">
          工作流 · {all.length}
        </span>
        <IconButton
          aria-label="新建流程"
          disabled={!ready || state.busy}
          onClick={() => {
            void studio.safely(() => studio.create());
          }}
        >
          <Plus size={13} />
        </IconButton>
        <IconButton
          aria-label="刷新工作流"
          disabled={!ready || state.busy}
          onClick={() => {
            void studio.safely(() => studio.refreshDocuments());
          }}
        >
          <RefreshCw size={12} />
        </IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {documents.map((item) => (
          <DocumentRow
            key={item.id}
            id={item.id}
            name={item.name}
            active={state.active === item.id}
            disabled={!ready || state.busy}
            locked={documentReadonly(state, item.id)}
            onMenu={(x, y) => setMenu({ id: item.id, x, y })}
            onRename={() => setAction({ kind: "rename", ...item })}
            onDelete={() => setAction({ kind: "delete", ...item })}
            onOpen={() => {
              void studio.safely(() => studio.open(item.id));
            }}
          />
        ))}
        {!documents.length && (
          <p className="px-2 py-4 text-xs leading-5 text-muted">
            {query
              ? "没有匹配的工作流"
              : ready
                ? "还没有工作流，点击上方 ＋ 新建。"
                : "正在准备工作流数据…"}
          </p>
        )}
      </div>
      {menu && current && (
        <Menu
          x={menu.x}
          y={menu.y}
          label="工作流操作菜单"
          onClose={closeMenu}
          items={[
            {
              type: "action",
              label: "打开",
              icon: <FolderOpen size={14} />,
              disabled: state.busy,
              action: () => {
                void studio.safely(() => studio.open(current.id));
              },
            },
            {
              type: "action",
              label: "重命名",
              icon: <Pencil size={14} />,
              shortcut: "F2",
              disabled: documentReadonly(state, current.id),
              reason: "工作流正在使用中",
              action: () => setAction({ kind: "rename", ...current }),
            },
            { type: "separator", id: "delete" },
            {
              type: "action",
              label: "删除",
              icon: <Trash2 size={14} />,
              shortcut: "Del",
              danger: true,
              disabled: documentReadonly(state, current.id),
              reason: "工作流正在使用中",
              action: () => setAction({ kind: "delete", ...current }),
            },
          ]}
        />
      )}
      {action && (
        <DocumentDialog action={action} onClose={() => setAction(null)} />
      )}
    </>
  );
}
