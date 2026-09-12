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
