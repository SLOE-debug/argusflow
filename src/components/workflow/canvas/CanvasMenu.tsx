import {
  ClipboardPaste,
  Copy,
  CopyPlus,
  Scissors,
  Trash2,
  Undo2,
  Redo2,
  Plus,
  Search,
  Unplug,
  LayoutPanelLeft,
} from "lucide-react";
import { useStore } from "zustand";
import type { FlowPoint } from "../../../flow";
import {
  studio,
  removeEdge,
  commonNodes,
  nodeUsage,
  type EditorTab,
} from "../../../features/workflow";
import { Menu, type MenuItem } from "../../ui";
import { NodeIcon } from "../presentation/NodeIcon";
import { arrangeMenu } from "./arrangeMenu";

/** 菜单的屏幕落点与对应的作用域坐标。 */
export interface CanvasMenuPosition {
  readonly scope: string;
  readonly x: number;
  readonly y: number;
  readonly point: FlowPoint;
  readonly edge?: string;
}

/** 画布菜单只装配业务命令，级联交互与定位由通用菜单负责。 */
export function CanvasMenu({
  tab,
  menu,
  onClose,
  onAdd,
}: {
  readonly tab: EditorTab;
  readonly menu: CanvasMenuPosition;
  readonly onClose: () => void;
  readonly onAdd: () => void;
}) {
  const usage = useStore(nodeUsage.store);
  const locked = studio.readonly;
  const selectionReason = locked ? "运行快照只读" : "请先选择节点";
  /** 二级菜单直接提供常用节点，完整节点库保留搜索入口。 */
  const nodes: readonly MenuItem[] = [
    ...commonNodes(usage.entries).map((node): MenuItem => ({
      type: "action",
      label: node.title,
      icon: <NodeIcon kind={node.id} size={14} />,
      action: () => {
        void studio.safely(() =>
          studio.add(
            node.id,
            menu.point,
            menu.edge ? { kind: "insert", edge: menu.edge } : undefined,
            menu.scope,
          ),
        );
      },
    })),
    { type: "separator", id: "search" },
    {
      type: "action",
      label: "搜索全部节点…",
      icon: <Search />,
      shortcut: "Tab",
      action: onAdd,
    },
  ];
  return (
    <Menu
      x={menu.x}
      y={menu.y}
      label="画布菜单"
      onClose={onClose}
      items={[
        {
          type: "action",
          label: "撤销",
          icon: <Undo2 />,
          shortcut: "Ctrl+Z",
          disabled: locked || !tab.past.length,
          reason: locked ? "运行快照只读" : "没有可撤销的操作",
          action: () => studio.undo(),
        },
        {
          type: "action",
          label: "重做",
          icon: <Redo2 />,
          shortcut: "Ctrl+Y",
          disabled: locked || !tab.future.length,
          reason: locked ? "运行快照只读" : "没有可重做的操作",
          action: () => studio.redo(),
        },
        { type: "separator", id: "edit" },
        {
          type: "action",
          label: "复制",
          icon: <Copy />,
          shortcut: "Ctrl+C",
          disabled: !tab.selected.length,
          reason: "请先选择节点",
          action: () => {
            void studio.safely(() => studio.copy());
          },
        },
        {
          type: "action",
          label: "剪切",
          icon: <Scissors />,
          shortcut: "Ctrl+X",
          disabled: locked || !tab.selected.length,
          reason: selectionReason,
          action: () => {
            void studio.safely(() => studio.copy(true));
          },
        },
        {
          type: "action",
          label: "粘贴",
          icon: <ClipboardPaste />,
          shortcut: "Ctrl+V",
          disabled: locked,
          reason: "运行快照只读",
          action: () => {
            void studio.safely(() => studio.paste(menu.point, menu.scope));
          },
        },
        {
          type: "action",
          label: "创建副本",
          icon: <CopyPlus />,
          shortcut: "Ctrl+D",
          disabled: locked || !tab.selected.length,
          reason: selectionReason,
          action: () => studio.duplicate(),
        },
        {
          type: "action",
          label: "删除",
          icon: <Trash2 />,
          shortcut: "Del",
          danger: true,
          disabled: locked || !tab.selected.length,
          reason: selectionReason,
          action: () => studio.remove(),
        },
        ...(menu.edge
          ? [
              {
                type: "action" as const,
                label: "删除连线",
                icon: <Unplug />,
                danger: true,
                disabled: locked,
                reason: "运行快照只读",
                action: () =>
                  studio.edit((file) =>
                    removeEdge(file, menu.scope, menu.edge!),
                  ),
              },
            ]
          : []),
        { type: "separator", id: "create" },
        {
          type: "submenu",
          label: menu.edge ? "在线路上插入节点" : "添加节点",
          icon: <Plus />,
          disabled: locked,
          reason: "运行快照只读",
          items: nodes,
        },
        {
          type: "submenu",
          label: "排列与对齐",
          icon: <LayoutPanelLeft />,
          disabled: locked,
          reason: "运行快照只读",
          items: arrangeMenu(tab.selected.length),
        },
      ]}
    />
  );
}
