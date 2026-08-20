import type { FlowNode } from './types';

export type AlignMode = 'left' | 'center-x' | 'right' | 'top' | 'center-y' | 'bottom';
export type DistributeMode = 'horizontal' | 'vertical';

/** 返回对齐后的节点副本，未选中节点保持引用不变。 */
export function alignNodes<T>(nodes: FlowNode<T>[], selectedIds: Set<string>, mode: AlignMode): FlowNode<T>[] {
  const selected = nodes.filter((node) => selectedIds.has(node.id));
  if (selected.length < 2) return nodes;
  const values = selected.map((node) => alignmentValue(node, mode));
  const target = mode === 'left' || mode === 'top' ? Math.min(...values) : mode === 'right' || mode === 'bottom' ? Math.max(...values) : values.reduce((sum, value) => sum + value, 0) / values.length;
  return nodes.map((node) => selectedIds.has(node.id) ? moveToAlignment(node, mode, target) : node);
}

/** 在首尾节点之间等距分布节点中心。 */
export function distributeNodes<T>(nodes: FlowNode<T>[], selectedIds: Set<string>, mode: DistributeMode): FlowNode<T>[] {
  const selected = nodes.filter((node) => selectedIds.has(node.id)).sort((a, b) => center(a, mode) - center(b, mode));
  if (selected.length < 3) return nodes;
  const start = center(selected[0], mode);
  const step = (center(selected.at(-1)!, mode) - start) / (selected.length - 1);
  const targets = new Map(selected.map((node, index) => [node.id, start + step * index]));
  return nodes.map((node) => {
    const target = targets.get(node.id);
    if (target === undefined) return node;
    return mode === 'horizontal'
      ? { ...node, position: { ...node.position, x: target - node.size.width / 2 } }
      : { ...node, position: { ...node.position, y: target - node.size.height / 2 } };
  });
}

function alignmentValue(node: FlowNode, mode: AlignMode): number {
  if (mode === 'left') return node.position.x;
  if (mode === 'right') return node.position.x + node.size.width;
  if (mode === 'top') return node.position.y;
  if (mode === 'bottom') return node.position.y + node.size.height;
  return mode === 'center-x' ? node.position.x + node.size.width / 2 : node.position.y + node.size.height / 2;
}

function moveToAlignment<T>(node: FlowNode<T>, mode: AlignMode, target: number): FlowNode<T> {
  const position = { ...node.position };
  if (mode === 'left') position.x = target;
  else if (mode === 'right') position.x = target - node.size.width;
  else if (mode === 'top') position.y = target;
  else if (mode === 'bottom') position.y = target - node.size.height;
  else if (mode === 'center-x') position.x = target - node.size.width / 2;
  else position.y = target - node.size.height / 2;
  return { ...node, position };
}

function center(node: FlowNode, mode: DistributeMode): number {
  return mode === 'horizontal' ? node.position.x + node.size.width / 2 : node.position.y + node.size.height / 2;
}
