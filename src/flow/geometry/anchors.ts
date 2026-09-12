import type { FlowAnchorSide, FlowPoint, FlowRect } from "../types";

/** 边中点携带朝外方向；自由指针端点没有所属障碍物。 */
export interface FlowAnchor {
  readonly point: FlowPoint;
  readonly side: FlowAnchorSide;
  readonly owner?: string;
}
export const ANCHOR_SIDES = ["top", "right", "bottom", "left"] as const;
export const ANCHOR_NORMALS: Readonly<Record<FlowAnchorSide, FlowPoint>> = {
  top: { x: 0, y: -1 },
  right: { x: 1, y: 0 },
  bottom: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
};
/** 指定边位不随其他节点移动而变化。 */
export function rectAnchor(
  rect: FlowRect,
  side: FlowAnchorSide,
  owner?: string,
): FlowAnchor {
  const normal = ANCHOR_NORMALS[side];
  return {
    owner,
    side,
    point: {
      x: rect.x + rect.width / 2 + (normal.x * rect.width) / 2,
      y: rect.y + rect.height / 2 + (normal.y * rect.height) / 2,
    },
  };
}
/** 自由端朝向另一端，正式吸附后由候选路径决定边位。 */
export function facingSide(point: FlowPoint, other: FlowPoint): FlowAnchorSide {
  const dx = other.x - point.x,
    dy = other.y - point.y;
  return Math.abs(dx) >= Math.abs(dy)
    ? dx >= 0
      ? "right"
      : "left"
    : dy >= 0
      ? "bottom"
      : "top";
}
