import {
  AlignStartVertical,
  AlignCenterVertical,
  AlignEndVertical,
  AlignStartHorizontal,
  AlignCenterHorizontal,
  AlignEndHorizontal,
  AlignHorizontalSpaceBetween,
  AlignVerticalSpaceBetween,
  LayoutGrid,
} from "lucide-react";
import { arrangeSelection, type Arrangement } from "../../../features/workflow";
import type { MenuItem } from "../../ui";

/** 排列菜单复用画布内核已有的六向对齐和均匀分布能力。 */
const ALIGN_ACTIONS = [
  { label: "左对齐", icon: AlignStartVertical, mode: "left", minimum: 2 },
  {
    label: "水平居中",
    icon: AlignCenterVertical,
    mode: "center-x",
    minimum: 2,
  },
  { label: "右对齐", icon: AlignEndVertical, mode: "right", minimum: 2 },
  { label: "顶部对齐", icon: AlignStartHorizontal, mode: "top", minimum: 2 },
  {
    label: "垂直居中",
    icon: AlignCenterHorizontal,
    mode: "center-y",
    minimum: 2,
  },
  { label: "底部对齐", icon: AlignEndHorizontal, mode: "bottom", minimum: 2 },
  {
    label: "水平分布",
    icon: AlignHorizontalSpaceBetween,
    mode: "horizontal",
    minimum: 3,
  },
  {
    label: "垂直分布",
    icon: AlignVerticalSpaceBetween,
    mode: "vertical",
    minimum: 3,
  },
] as const satisfies readonly {
  label: string;
  icon: typeof LayoutGrid;
  mode: Arrangement;
  minimum: number;
}[];

/** 根据选择数量生成可用性说明，菜单本身不修改节点。 */
export function arrangeMenu(count: number): readonly MenuItem[] {
  return [
    ...ALIGN_ACTIONS.flatMap((item, index): MenuItem[] => [
      ...(index === 3 || index === 6
        ? [{ type: "separator" as const, id: item.mode }]
        : []),
      {
        type: "action",
        label: item.label,
        icon: <item.icon />,
        disabled: count < item.minimum,
        reason: item.minimum === 2 ? "至少选择两个节点" : "至少选择三个节点",
        action: () => arrangeSelection(item.mode),
      },
    ]),
    { type: "separator", id: "tidy" },
    {
      type: "action",
      label: "整理当前作用域",
      icon: <LayoutGrid />,
      action: () => arrangeSelection("tidy"),
    },
  ];
}
