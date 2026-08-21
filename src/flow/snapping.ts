import type { FlowNode, FlowPoint } from './types';

/** 节点在单一轴上可参与吸附的边缘或中心。 */
export type AlignmentKind = 'start' | 'center' | 'end';

/** 吸附线用于表达边缘对齐、中心对齐或节点间距。 */
export type AlignmentGuideKind = AlignmentKind | 'spacing';

/** WinForms 式动态吸附线，线段仅连接当前节点和参照节点。 */
export type AlignmentGuide = Readonly<{
  /** x 表示竖向吸附线，y 表示横向吸附线。 */
  axis: 'x' | 'y';
  /** 吸附线所在的世界坐标。 */
  value: number;
  /** 吸附线在另一坐标轴上的起点。 */
  start: number;
  /** 吸附线在另一坐标轴上的终点。 */
  end: number;
  /** 当前吸附的语义。 */
  kind: AlignmentGuideKind;
}>;

type Axis = 'x' | 'y';

type AxisInterval = Readonly<{
  end: number;
  start: number;
}>;

type AxisSnap = Readonly<{
  /** 应用到当前节点的位移。 */
  delta: number;
  /** 该吸附候选的可视反馈。 */
  guide: AlignmentGuide;
  /** 同等位移下，优先选择空间上更近的节点。 */
  referenceDistance: number;
}>;

/** 流程节点之间的建议留白，与画布基础网格保持一致。 */
const RECOMMENDED_NODE_GAP = 24;

/** 根据其他节点的边缘、中心和建议间距计算动态吸附。 */
export function snapNode(
  node: FlowNode,
  others: ReadonlyArray<FlowNode>,
  threshold: number,
): Readonly<{ position: FlowPoint; guides: ReadonlyArray<AlignmentGuide> }> {
  const xSnap = findAxisSnap(node, others, 'x', threshold);
  const ySnap = findAxisSnap(node, others, 'y', threshold);
  const candidateGuides = [
    ...(xSnap ? [xSnap.guide] : []),
    ...(ySnap ? [ySnap.guide] : []),
  ];
  const spacingGuides = candidateGuides.filter((guide) => guide.kind === 'spacing');
  /** 进入建议间距时只强调节点间距；继续靠近后立即清除反馈。 */
  const guides = spacingGuides.length > 0
    ? spacingGuides
    : hasCompressedNeighbor(node, others, threshold)
      ? []
      : candidateGuides;

  return {
    position: {
      x: node.position.x + (xSnap?.delta ?? 0),
      y: node.position.y + (ySnap?.delta ?? 0),
    },
    guides,
  };
}

/** 找出单一移动轴上位移最小、参照节点最近的吸附候选。 */
function findAxisSnap(
  node: FlowNode,
  others: ReadonlyArray<FlowNode>,
  axis: Axis,
  threshold: number,
): AxisSnap | null {
  let best: AxisSnap | null = null;

  for (const other of others) {
    const candidates = [
      ...alignmentCandidates(node, other, axis),
      ...spacingCandidates(node, other, axis),
    ];

    for (const candidate of candidates) {
      if (Math.abs(candidate.delta) > threshold) continue;
      if (isBetterCandidate(candidate, best)) best = candidate;
    }
  }

  return best;
}

/** 生成左、中、右或上、中、下的同语义对齐候选。 */
function alignmentCandidates(
  node: FlowNode,
  other: FlowNode,
  axis: Axis,
): ReadonlyArray<AxisSnap> {
  const referenceDistance = Math.abs(
    crossAxisCenter(other, axis) - crossAxisCenter(node, axis),
  );

  return axisValues(node, axis).map((from) => {
    const to = axisValues(other, axis).find((value) => value.kind === from.kind);
    if (!to) throw new Error(`Missing alignment value: ${from.kind}`);

    const delta = to.value - from.value;
    return {
      delta,
      guide: createAlignmentGuide(node, other, axis, to.value, from.kind),
      referenceDistance,
    };
  });
}

/** 生成相邻节点达到建议间距时的间距吸附候选。 */
function spacingCandidates(
  node: FlowNode,
  other: FlowNode,
  axis: Axis,
): ReadonlyArray<AxisSnap> {
  const crossAxis = axis === 'x' ? 'y' : 'x';
  const nodeCrossInterval = nodeInterval(node, crossAxis);
  const otherCrossInterval = nodeInterval(other, crossAxis);
  const overlap = intervalOverlap(nodeCrossInterval, otherCrossInterval);
  if (!overlap) return [];

  const nodeIntervalOnAxis = nodeInterval(node, axis);
  const otherIntervalOnAxis = nodeInterval(other, axis);
  const nodeBeforeOther = intervalCenter(nodeIntervalOnAxis)
    < intervalCenter(otherIntervalOnAxis);
  const targetStart = nodeBeforeOther
    ? otherIntervalOnAxis.start - RECOMMENDED_NODE_GAP
      - (nodeIntervalOnAxis.end - nodeIntervalOnAxis.start)
    : otherIntervalOnAxis.end + RECOMMENDED_NODE_GAP;
  const delta = targetStart - nodeIntervalOnAxis.start;
  const snappedInterval: AxisInterval = {
    start: nodeIntervalOnAxis.start + delta,
    end: nodeIntervalOnAxis.end + delta,
  };
  const guideStart = nodeBeforeOther
    ? snappedInterval.end
    : otherIntervalOnAxis.end;
  const guideEnd = nodeBeforeOther
    ? otherIntervalOnAxis.start
    : snappedInterval.start;

  return [{
    delta,
    guide: {
      axis: axis === 'x' ? 'y' : 'x',
      value: intervalCenter(overlap),
      start: Math.min(guideStart, guideEnd),
      end: Math.max(guideStart, guideEnd),
      kind: 'spacing',
    },
    referenceDistance: Math.abs(
      intervalCenter(nodeIntervalOnAxis) - intervalCenter(otherIntervalOnAxis),
    ),
  }];
}

/** 优先更小的吸附位移；同等位移时选择空间上更近的参照节点。 */
function isBetterCandidate(candidate: AxisSnap, current: AxisSnap | null): boolean {
  if (!current) return true;
  const deltaDifference = Math.abs(candidate.delta) - Math.abs(current.delta);
  if (Math.abs(deltaDifference) > Number.EPSILON) return deltaDifference < 0;
  return candidate.referenceDistance < current.referenceDistance;
}

/** 返回节点在指定轴上的起点、中心和终点。 */
function axisValues(
  node: FlowNode,
  axis: Axis,
): ReadonlyArray<Readonly<{ kind: AlignmentKind; value: number }>> {
  const interval = nodeInterval(node, axis);
  return [
    { kind: 'start', value: interval.start },
    { kind: 'center', value: intervalCenter(interval) },
    { kind: 'end', value: interval.end },
  ];
}

/** 返回节点在指定轴上的闭区间。 */
function nodeInterval(node: FlowNode, axis: Axis): AxisInterval {
  const start = node.position[axis];
  const size = axis === 'x' ? node.size.width : node.size.height;
  return { start, end: start + size };
}

/** 返回轴区间中心。 */
function intervalCenter(interval: AxisInterval): number {
  return (interval.start + interval.end) / 2;
}

/** 返回两个轴区间的交集，无交集时不生成间距吸附。 */
function intervalOverlap(
  first: AxisInterval,
  second: AxisInterval,
): AxisInterval | null {
  const start = Math.max(first.start, second.start);
  const end = Math.min(first.end, second.end);
  return end >= start ? { start, end } : null;
}

/** 检测已经穿过建议间距触发带的相邻节点。 */
function hasCompressedNeighbor(
  node: FlowNode,
  others: ReadonlyArray<FlowNode>,
  threshold: number,
): boolean {
  const compressedGap = RECOMMENDED_NODE_GAP - threshold;
  return others.some((other) => {
    const horizontalNeighbors = intervalOverlap(
      nodeInterval(node, 'y'),
      nodeInterval(other, 'y'),
    ) !== null;
    const verticalNeighbors = intervalOverlap(
      nodeInterval(node, 'x'),
      nodeInterval(other, 'x'),
    ) !== null;

    return (horizontalNeighbors && intervalGap(
      nodeInterval(node, 'x'),
      nodeInterval(other, 'x'),
    ) < compressedGap) || (verticalNeighbors && intervalGap(
      nodeInterval(node, 'y'),
      nodeInterval(other, 'y'),
    ) < compressedGap);
  });
}

/** 返回两个区间的空白距离，投影重叠时距离为零。 */
function intervalGap(first: AxisInterval, second: AxisInterval): number {
  if (first.end <= second.start) return second.start - first.end;
  if (second.end <= first.start) return first.start - second.end;
  return 0;
}

/** 返回另一坐标轴上的节点中心。 */
function crossAxisCenter(node: FlowNode, axis: Axis): number {
  return intervalCenter(nodeInterval(node, axis === 'x' ? 'y' : 'x'));
}

/** 构造一条连接两个对齐节点边界的吸附线。 */
function createAlignmentGuide(
  node: FlowNode,
  other: FlowNode,
  axis: Axis,
  value: number,
  kind: AlignmentKind,
): AlignmentGuide {
  const crossAxis = axis === 'x' ? 'y' : 'x';
  const span = intervalBetweenOrAround(
    nodeInterval(node, crossAxis),
    nodeInterval(other, crossAxis),
  );

  return {
    axis,
    value,
    start: span.start,
    end: span.end,
    kind,
  };
}

/** 分离的节点只连接中间空白；投影重叠时覆盖完整对齐范围。 */
function intervalBetweenOrAround(
  first: AxisInterval,
  second: AxisInterval,
): AxisInterval {
  if (first.end <= second.start) return { start: first.end, end: second.start };
  if (second.end <= first.start) return { start: second.end, end: first.start };
  return {
    start: Math.min(first.start, second.start),
    end: Math.max(first.end, second.end),
  };
}
