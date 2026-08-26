import { anchorPoint } from './geometry';
import { ROUTING_OBSTACLE_GAP } from './obstacleIndex';
import type {
  FlowAnchorSide,
  FlowEdge,
  FlowNode,
  FlowPoint,
  FlowRect,
  RoutedEdge,
} from './types';
import type { RoutingPort } from './routingTypes';

/** 端口出口位于节点膨胀安全区外一像素，避免主体接触禁止边界。 */
export const ENDPOINT_CLEARANCE = ROUTING_OBSTACLE_GAP + 1;

/** 自动端口选择时允许评估的稳定方向顺序。 */
const ROUTING_SIDES = [
  'top',
  'right',
  'bottom',
  'left',
] as const satisfies ReadonlyArray<FlowAnchorSide>;

/** 一对源、目标端口候选。 */
export type RoutingPortCandidate = Readonly<{
  /** 候选源端口方向。 */
  sourceSide: FlowAnchorSide;
  /** 候选目标端口方向。 */
  targetSide: FlowAnchorSide;
}>;

/** 从节点边界和指定侧构造锚点及其法线方向安全出口。 */
export function buildRoutingPort(
  node: FlowNode,
  side: FlowAnchorSide,
): RoutingPort {
  const rect = nodeRect(node);
  const anchor = anchorPoint(rect, side);
  return {
    nodeId: node.id,
    side,
    anchor,
    escape: offsetFromAnchor(anchor, side),
  };
}

/** 枚举满足显式 side 硬约束的端口组合，并按方向与稳定性排序。 */
export function routingPortCandidates(
  edge: FlowEdge,
  source: FlowNode,
  target: FlowNode,
  previous?: RoutedEdge,
): ReadonlyArray<RoutingPortCandidate> {
  const sourceSides = edge.source.side
    ? [edge.source.side]
    : ROUTING_SIDES;
  const targetSides = edge.target.side
    ? [edge.target.side]
    : ROUTING_SIDES;
  /** 组合数量最多为 16，排序只发生在单条脏边的端口决策中。 */
  const candidates: RoutingPortCandidate[] = [];
  for (const sourceSide of sourceSides) {
    for (const targetSide of targetSides) {
      candidates.push({ sourceSide, targetSide });
    }
  }
  return candidates.sort((a, b) => (
    portCandidateScore(a, source, target, previous)
    - portCandidateScore(b, source, target, previous)
  ));
}

/** 把路由主体夹在两个端口的强制直线段之间。 */
export function joinRoutingPorts(
  source: RoutingPort,
  target: RoutingPort,
  corePoints: ReadonlyArray<FlowPoint>,
): ReadonlyArray<FlowPoint> {
  return [
    source.anchor,
    source.escape,
    ...corePoints,
    target.escape,
    target.anchor,
  ];
}

/** 返回节点未膨胀的真实边界。 */
export function nodeRect(node: FlowNode): FlowRect {
  return { ...node.position, ...node.size };
}

/** 沿锚点所在边的外法线移动到允许转弯的位置。 */
function offsetFromAnchor(
  point: FlowPoint,
  side: FlowAnchorSide,
): FlowPoint {
  switch (side) {
    case 'top':
      return { x: point.x, y: point.y - ENDPOINT_CLEARANCE };
    case 'right':
      return { x: point.x + ENDPOINT_CLEARANCE, y: point.y };
    case 'bottom':
      return { x: point.x, y: point.y + ENDPOINT_CLEARANCE };
    case 'left':
      return { x: point.x - ENDPOINT_CLEARANCE, y: point.y };
  }
}

/** 自动端口排序同时考虑路径朝向与上一条路线的方向滞回。 */
function portCandidateScore(
  candidate: RoutingPortCandidate,
  source: FlowNode,
  target: FlowNode,
  previous?: RoutedEdge,
): number {
  const sourceAnchor = anchorPoint(nodeRect(source), candidate.sourceSide);
  const targetAnchor = anchorPoint(nodeRect(target), candidate.targetSide);
  const directionPenalty = outwardPenalty(
    sourceAnchor,
    targetAnchor,
    candidate.sourceSide,
  ) + outwardPenalty(
    targetAnchor,
    sourceAnchor,
    candidate.targetSide,
  );
  const changedPreviousSide = previous
    && (
      previous.sourceSide !== candidate.sourceSide
      || previous.targetSide !== candidate.targetSide
    );
  const hysteresis = changedPreviousSide ? 36 : 0;
  return Math.abs(sourceAnchor.x - targetAnchor.x)
    + Math.abs(sourceAnchor.y - targetAnchor.y)
    + directionPenalty
    + hysteresis;
}

/** 逆着端口外法线连接时施加惩罚，但不改变显式 side 约束。 */
function outwardPenalty(
  from: FlowPoint,
  to: FlowPoint,
  side: FlowAnchorSide,
): number {
  const followsNormal = side === 'right' && to.x >= from.x
    || side === 'left' && to.x <= from.x
    || side === 'bottom' && to.y >= from.y
    || side === 'top' && to.y <= from.y;
  return followsNormal ? 0 : 160;
}
