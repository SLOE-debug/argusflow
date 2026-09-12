import type { FlowRect } from "../../../../flow";
import type { ThemeDefinition } from "../../../../features/themes";
import type { NodeRunState } from "../../execution/nodeStates";
import type { NodePresentation } from "../scene";
import type { DrawingResources } from "./resources";

/** 绘制圆角路径，调用方负责填充与描边。 */
export function rounded(
  context: CanvasRenderingContext2D,
  rect: FlowRect,
  radius = 12,
): void {
  context.beginPath();
  context.roundRect(rect.x, rect.y, rect.width, rect.height, radius);
}

/** 在局部坐标中绘制卡片，不处理工作流状态变更。 */
export function drawCard(
  context: CanvasRenderingContext2D,
  resources: DrawingResources,
  rect: FlowRect,
  node: NodePresentation,
  colors: ThemeDefinition["colors"],
  zoom: number,
  selected: boolean,
  container: boolean,
  state?: NodeRunState,
): void {
  const tone = state === "failed" ? colors.danger : colors[node.tone];
  rounded(context, rect);
  context.fillStyle = colors.surface;
  context.shadowColor = colors.border;
  context.shadowBlur = zoom >= 0.3 && zoom <= 3 ? 12 : 0;
  context.shadowOffsetY = 3;
  context.fill();
  context.shadowBlur = 0;
  context.shadowOffsetY = 0;
  context.strokeStyle = selected ? colors.accent : tone;
  context.globalAlpha = selected ? 1 : 0.45;
  context.lineWidth = selected ? 2 : 1;
  context.stroke();
  context.globalAlpha = 1;
  if (selected) {
    context.globalAlpha = 0.12;
    context.lineWidth = 6;
    context.stroke();
    context.globalAlpha = 1;
  }
  if (container) {
    context.fillStyle = colors.structureSoft;
    context.globalAlpha = 0.35;
    context.fill();
    context.globalAlpha = 1;
  }
  // 远景仅保留轮廓；极近的祖先标题也无需栅格化。
  if (zoom < 0.25 || zoom > 16) return;
  const centerY = rect.y + (container ? 22 : rect.height / 2);
  context.beginPath();
  context.arc(rect.x + 32, centerY, container ? 15 : 21, 0, Math.PI * 2);
  context.fillStyle = tone;
  context.globalAlpha = 0.1;
  context.fill();
  context.globalAlpha = 1;
  resources.icon(context, node.kind, tone, rect.x + 20, centerY - 12);
  context.textBaseline = "middle";
  context.fillStyle = colors.text;
  context.font = "600 14px system-ui, sans-serif";
  context.fillText(
    resources.label(context, node.title, rect.width - 98),
    rect.x + 64,
    centerY - (container ? 0 : 10),
  );
  if (!container && zoom >= 0.4) {
    context.font = "12px system-ui, sans-serif";
    context.fillStyle = colors.muted;
    context.fillText(
      resources.label(context, node.summary, rect.width - 82),
      rect.x + 64,
      centerY + 12,
    );
  }
  context.fillStyle = colors.muted;
  for (let i = 0; i < 3; i++) {
    context.beginPath();
    context.arc(
      rect.x + rect.width - 26 + i * 5,
      rect.y + 18,
      1.3,
      0,
      Math.PI * 2,
    );
    context.fill();
  }
  if (state) {
    const labels = {
      running: "执行中",
      completed: "已完成",
      waiting: "等待执行",
      failed: "失败",
    };
    context.font = "10px system-ui, sans-serif";
    context.fillStyle =
      state === "failed"
        ? colors.danger
        : state === "completed"
          ? colors.success
          : colors.accent;
    context.textAlign = "right";
    context.fillText(
      labels[state],
      rect.x + rect.width - 12,
      rect.y + rect.height - 10,
    );
    context.textAlign = "left";
  }
}

/** 起止卡片使用同一套卡片尺寸和显式图形符号。 */
export function drawEndpoint(
  context: CanvasRenderingContext2D,
  resources: DrawingResources,
  rect: FlowRect,
  entry: boolean,
  colors: ThemeDefinition["colors"],
  zoom: number,
  selected: boolean,
): void {
  const tone = entry ? colors.nodeStart : colors.nodeEnd;
  rounded(context, rect);
  context.fillStyle = colors.surface;
  context.fill();
  context.strokeStyle = selected ? colors.accent : tone;
  context.globalAlpha = selected ? 1 : 0.35;
  context.lineWidth = selected ? 2 : 1;
  context.stroke();
  context.globalAlpha = 1;
  if (zoom < 0.25 || zoom > 16) return;
  const centerY = rect.y + rect.height / 2;
  context.beginPath();
  context.arc(rect.x + 32, centerY, 20, 0, Math.PI * 2);
  context.fillStyle = tone;
  context.globalAlpha = 0.07;
  context.fill();
  context.globalAlpha = 1;
  resources.icon(
    context,
    entry ? "start" : "end",
    tone,
    rect.x + 20,
    centerY - 12,
  );
  context.textBaseline = "middle";
  context.fillStyle = colors.text;
  context.font = "600 14px system-ui, sans-serif";
  context.fillText(entry ? "开始" : "结束", rect.x + 62, centerY - 10);
  context.fillStyle = colors.muted;
  context.font = "12px system-ui, sans-serif";
  context.fillText(
    entry ? "工作流入口" : "工作流出口",
    rect.x + 62,
    centerY + 12,
  );
}

/** 端口的绘制尺寸与命中区域独立，保留小尺度可操作性。 */
export function drawPort(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  color: string,
  background: string,
  zoom: number,
): void {
  if (zoom < 0.25) return;
  context.beginPath();
  context.arc(x, y, 4 / zoom, 0, Math.PI * 2);
  context.fillStyle = background;
  context.fill();
  context.strokeStyle = color;
  context.lineWidth = 1.5 / zoom;
  context.stroke();
}
