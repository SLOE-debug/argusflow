import { memo, type PointerEvent as ReactPointerEvent } from 'react';

import { anchorPoint } from '../geometry/geometry';
import type { CanvasToolMode } from './FlowCanvasTools';
import { findFlowNode } from '../selection/nodeLookup';
import { useFlowStore } from '../store/store';
import type { FlowAnchorSide, FlowNode, FlowPoint, NodeRegistry } from '../types';
import { useNodeResizeGesture } from '../interaction/useNodeResizeGesture';

const ANCHOR_SIDES = [
  'top',
  'right',
  'bottom',
  'left',
] as const satisfies ReadonlyArray<FlowAnchorSide>;

/** 锚点在节点四边上的绝对定位。 */
const ANCHOR_POSITIONS = {
  top: 'left-1/2 top-0',
  right: 'left-full top-1/2',
  bottom: 'left-1/2 top-full',
  left: 'left-0 top-1/2',
} as const satisfies Readonly<Record<FlowAnchorSide, string>>;

/** 选中框视觉样式。 */
const SELECTION_OUTLINE_CLASS_NAME = [
  'pointer-events-none absolute -inset-1 rounded-[8px] border',
  'border-blue-500 ring-2 ring-blue-500/10',
].join(' ');

/** 连线锚点视觉样式；空心菱形比实心圆更轻，并保持明确连接语义。 */
const ANCHOR_CLASS_NAME = [
  'absolute z-30 size-2 -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[2px]',
  'border border-blue-500 bg-white p-0 shadow-sm outline-none',
  'transition-[transform,background-color] hover:scale-125 hover:bg-blue-50',
].join(' ');

type FlowNodeViewProps = Readonly<{
  nodeId: string;
  registry: Readonly<NodeRegistry>;
  toolMode: CanvasToolMode;
  /** 平移手势是否覆盖节点选择和拖动。 */
  panActive: boolean;
  onDragStart: (nodeId: string, event: ReactPointerEvent) => void;
  onConnectionStart: (
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
  /** 业务层可选择响应节点双击，例如进入嵌套流程。 */
  onDoubleClick?: (nodeId: string) => void;
}>;

/** 单节点订阅组件，节点变化不会触发其他节点 React 重渲染。 */
export const FlowNodeView = memo(function FlowNodeView({
  nodeId,
  registry,
  toolMode,
  panActive,
  onDragStart,
  onConnectionStart,
  onDoubleClick,
}: FlowNodeViewProps) {
  const node = useFlowStore((state) => findFlowNode(state.nodes, nodeId));
  const selected = useFlowStore((state) => state.selectedNodeIds.has(nodeId));
  const hovered = useFlowStore((state) => state.hoveredNodeId === nodeId);
  const selectNodes = useFlowStore((state) => state.selectNodes);
  const setHoveredNode = useFlowStore((state) => state.setHoveredNode);

  const definition = node ? registry[node.kind] : undefined;
  const startResize = useNodeResizeGesture({
    node,
    minSize: definition?.minSize ?? definition?.defaultSize ?? { width: 1, height: 1 },
  });
  if (!node || !definition) return null;

  const NodeComponent = definition.component;
  const handlePointerEnter = () => setHoveredNode(node.id);
  const handlePointerLeave = () => setHoveredNode(null);
  const handlePointerDown = (event: ReactPointerEvent) => {
    if (panActive) return;

    const target = event.target;
    if (
      target instanceof HTMLElement
      && target.closest('[data-flow-anchor], [data-flow-resize]')
    ) return;

    event.stopPropagation();
    const selectionMode = event.ctrlKey || event.metaKey
      ? 'toggle'
      : event.shiftKey || selected
        ? 'add'
        : 'replace';
    selectNodes([node.id], selectionMode);
    onDragStart(node.id, event);
  };

  return (
    <div
      className={`pointer-events-auto absolute select-none ${panActive ? 'cursor-grab' : 'cursor-move'}`}
      data-flow-node-id={node.id}
      style={{
        height: node.size.height,
        transform: `translate(${node.position.x}px, ${node.position.y}px)`,
        width: node.size.width,
      }}
      onPointerDown={handlePointerDown}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
      onDoubleClick={(event) => {
        event.stopPropagation();
        onDoubleClick?.(node.id);
      }}
    >
      <NodeComponent
        node={node}
        selected={selected}
      />
      {selected ? <NodeSelectionOutline /> : null}
      {selected && definition.resizable && !panActive ? (
        <button
          type="button"
          aria-label={`调整 ${node.id} 大小`}
          className="absolute right-0 bottom-0 z-40 size-4 translate-x-1/2 translate-y-1/2 cursor-nwse-resize rounded-sm border border-violet-500 bg-white shadow-sm hover:bg-violet-50"
          data-flow-resize="true"
          onPointerDown={startResize}
        />
      ) : null}
      {hovered
      && toolMode === 'select'
      && !panActive
      && definition.canStartConnection !== false ? (
        <NodeAnchors
          node={node}
          onConnectionStart={onConnectionStart}
        />
      ) : null}
    </div>
  );
});

/** 绘制选中节点的高亮边框。 */
function NodeSelectionOutline() {
  return (
    <div className={SELECTION_OUTLINE_CLASS_NAME} />
  );
}

type NodeAnchorsProps = Readonly<{
  node: FlowNode;
  onConnectionStart: (
    nodeId: string,
    side: FlowAnchorSide,
    point: FlowPoint,
    event: ReactPointerEvent,
  ) => void;
}>;

/** 绘制节点四边的连线锚点，并把绝对锚点坐标交给画布手势层。 */
function NodeAnchors({ node, onConnectionStart }: NodeAnchorsProps) {
  const nodeRect = { ...node.position, ...node.size };

  return ANCHOR_SIDES.map((side) => {
    const point = anchorPoint(nodeRect, side);
    const startConnection = (event: ReactPointerEvent) => {
      onConnectionStart(node.id, side, point, event);
    };

    return (
      <button
        key={side}
        type="button"
        aria-label={`${node.id} ${side} 锚点`}
        className={`${ANCHOR_CLASS_NAME} ${ANCHOR_POSITIONS[side]}`}
        data-anchor-side={side}
        data-flow-anchor="true"
        onPointerDown={startConnection}
      />
    );
  });
}
