import type { FlowNode } from "../types";

export type AlignMode =
  "left" | "center-x" | "right" | "top" | "center-y" | "bottom";
export type DistributeMode = "horizontal" | "vertical";

/** 返回对齐后的节点副本，未选中节点保持引用不变。 */
export function alignNodes<T>(
  nodes: ReadonlyArray<FlowNode<T>>,
  selectedIds: ReadonlySet<string>,
  mode: AlignMode,
): ReadonlyArray<FlowNode<T>> {
  const selected = nodes.filter((node) => selectedIds.has(node.id));
  if (selected.length < 2) return nodes;
  const values = selected.map((node) => alignmentValue(node, mode));
  const target =
    mode === "left" || mode === "top"
      ? Math.min(...values)
      : mode === "right" || mode === "bottom"
        ? Math.max(...values)
        : values.reduce((sum, value) => sum + value, 0) / values.length;
  const aligned = nodes.map((node) =>
    selectedIds.has(node.id) ? moveToAlignment(node, mode, target) : node,
  );
  return aligned.every((node, index) => node === nodes[index])
    ? nodes
    : aligned;
}

/** 按节点边缘等距分布；空间不足时扩展末端，避免节点重叠。 */
export function distributeNodes<T>(
  nodes: ReadonlyArray<FlowNode<T>>,
  selectedIds: ReadonlySet<string>,
  mode: DistributeMode,
): ReadonlyArray<FlowNode<T>> {
  const selected = nodes
    .filter((node) => selectedIds.has(node.id))
    .sort((a, b) => leading(a, mode) - leading(b, mode));
  if (selected.length < 3) return nodes;
  const start = leading(selected[0], mode);
  const last = selected.at(-1)!;
  const occupied = selected.reduce((sum, node) => sum + extent(node, mode), 0);
  /** 剩余空间均分为节点之间的净间距，而非中心距离。 */
  const gap = Math.max(
    0,
    (leading(last, mode) + extent(last, mode) - start - occupied) /
      (selected.length - 1),
  );
  const targets = new Map<string, number>();
  let cursor = start;
  for (const node of selected) {
    targets.set(node.id, cursor);
    cursor += extent(node, mode) + gap;
  }
  const distributed = nodes.map((node) => {
    const target = targets.get(node.id);
    if (target === undefined) return node;
    const nextPosition =
      mode === "horizontal"
        ? { ...node.position, x: target }
        : { ...node.position, y: target };
    return nextPosition.x === node.position.x &&
      nextPosition.y === node.position.y
      ? node
      : { ...node, position: nextPosition };
  });
  return distributed.every((node, index) => node === nodes[index])
    ? nodes
    : distributed;
}

function alignmentValue(node: FlowNode, mode: AlignMode): number {
  if (mode === "left") return node.position.x;
  if (mode === "right") return node.position.x + node.size.width;
  if (mode === "top") return node.position.y;
  if (mode === "bottom") return node.position.y + node.size.height;
  return mode === "center-x"
    ? node.position.x + node.size.width / 2
    : node.position.y + node.size.height / 2;
}

function moveToAlignment<T>(
  node: FlowNode<T>,
  mode: AlignMode,
  target: number,
): FlowNode<T> {
  const position = { ...node.position };
  if (mode === "left") position.x = target;
  else if (mode === "right") position.x = target - node.size.width;
  else if (mode === "top") position.y = target;
  else if (mode === "bottom") position.y = target - node.size.height;
  else if (mode === "center-x") position.x = target - node.size.width / 2;
  else position.y = target - node.size.height / 2;
  return position.x === node.position.x && position.y === node.position.y
    ? node
    : { ...node, position };
}

function leading(node: FlowNode, mode: DistributeMode): number {
  return mode === "horizontal" ? node.position.x : node.position.y;
}

function extent(node: FlowNode, mode: DistributeMode): number {
  return mode === "horizontal" ? node.size.width : node.size.height;
}
