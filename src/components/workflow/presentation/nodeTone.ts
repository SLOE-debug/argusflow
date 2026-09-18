import { nodeColor } from "../../../features/workflow";
/** 类别映射为语义颜色，主题定义负责实际色值。 */
export function nodeTone(kind: string): string {
  return {
    nodeStart: "text-node-start",
    nodeEnd: "text-node-end",
    data: "text-data",
    browser: "text-browser",
    desktop: "text-desktop",
    structure: "text-structure",
    accent: "text-accent",
  }[nodeColor(kind)];
}

/** 类型标签使用类别底色和白字，与节点图标保持同一颜色语义。 */
export function nodeBadgeTone(kind: string): string {
  return {
    nodeStart: "bg-node-start text-white",
    nodeEnd: "bg-node-end text-white",
    data: "bg-data text-white",
    browser: "bg-browser text-white",
    desktop: "bg-desktop text-white",
    structure: "bg-structure text-white",
    accent: "bg-accent text-white",
  }[nodeColor(kind)];
}
