import { NODE_CATALOG } from "../../../features/workflow";
/** 类别映射为语义颜色，主题定义负责实际色值。 */
export function nodeTone(kind: string): string {
  const category = NODE_CATALOG.find((item) => item.id === kind)?.category;
  switch (category) {
    case "数据":
      return "text-data";
    case "浏览器":
      return "text-browser";
    case "桌面自动化":
      return "text-desktop";
    case "逻辑控制":
      return "text-structure";
    default:
      return "text-accent";
  }
}
