import { NODE_CATALOG } from "./catalog";

/** 节点语义颜色的唯一来源，DOM 图标与 Canvas 共同使用。 */
export function nodeColor(kind: string) {
  if (kind === "start") return "nodeStart";
  if (kind === "end") return "nodeEnd";
  switch (NODE_CATALOG.find((item) => item.id === kind)?.category) {
    case "数据":
      return "data";
    case "浏览器":
      return "browser";
    case "桌面自动化":
      return "desktop";
    case "逻辑控制":
      return "structure";
    default:
      return "accent";
  }
}
