import type { FlowNode, FlowPoint } from './types';

export type AlignmentGuide = { axis: 'x' | 'y'; value: number };

/** 根据其他节点的边和中心计算拖拽吸附位移与辅助线。 */
export function snapNode(node: FlowNode, others: FlowNode[], threshold: number): { position: FlowPoint; guides: AlignmentGuide[] } {
  const xValues = [node.position.x, node.position.x + node.size.width / 2, node.position.x + node.size.width];
  const yValues = [node.position.y, node.position.y + node.size.height / 2, node.position.y + node.size.height];
  let bestX: { delta: number; value: number } | null = null;
  let bestY: { delta: number; value: number } | null = null;
  for (const other of others) {
    const otherX = [other.position.x, other.position.x + other.size.width / 2, other.position.x + other.size.width];
    const otherY = [other.position.y, other.position.y + other.size.height / 2, other.position.y + other.size.height];
    for (const from of xValues) for (const to of otherX) if (Math.abs(to - from) <= threshold && (!bestX || Math.abs(to - from) < Math.abs(bestX.delta))) bestX = { delta: to - from, value: to };
    for (const from of yValues) for (const to of otherY) if (Math.abs(to - from) <= threshold && (!bestY || Math.abs(to - from) < Math.abs(bestY.delta))) bestY = { delta: to - from, value: to };
  }
  return {
    position: { x: node.position.x + (bestX?.delta ?? 0), y: node.position.y + (bestY?.delta ?? 0) },
    guides: [...(bestX ? [{ axis: 'x' as const, value: bestX.value }] : []), ...(bestY ? [{ axis: 'y' as const, value: bestY.value }] : [])],
  };
}
