import { useCallback, type PointerEvent as ReactPointerEvent } from 'react';

import { findFlowNode } from '../selection/nodeLookup';
import { useFlowStoreApi } from '../store/store';
import type { FlowNode } from '../types';
import { bindPointerGesture, createAnimationFrameCoalescer } from './pointerGesture';

type NodeResizeOptions = Readonly<{
  node: FlowNode | undefined;
  minSize: Readonly<{ width: number; height: number }>;
}>;

/**
 * 调整单个结构容器尺寸；移动帧只更新活动文档，结束时把整次手势压成一次全局撤销。
 */
export function useNodeResizeGesture({
  node,
  minSize,
}: NodeResizeOptions): (event: ReactPointerEvent) => void {
  const store = useFlowStoreApi();
  return useCallback((event: ReactPointerEvent) => {
    if (event.button !== 0 || !node) return;
    event.preventDefault();
    event.stopPropagation();

    /** 屏幕位移需要按手势开始时的倍率换算成稳定世界尺寸。 */
    const initialZoom = store.getState().viewport.zoom;
    const initialPointer = { x: event.clientX, y: event.clientY };
    const initialState = store.getState();
    const initialNodes = initialState.nodes;
    const initialEdges = initialState.edges;
    const initialMetadata = initialState.metadata;
    const initialDocuments = initialState.documents;
    const initialActiveDocumentId = initialState.activeDocumentId;

    /** 将本帧最后位置应用到初始节点，避免累计舍入误差。 */
    const applyResizeFrame = (pointer: PointerEvent) => {
      const current = findFlowNode(store.getState().nodes, node.id);
      if (!current) return;
      const size = {
        width: Math.max(
          minSize.width,
          Math.round(node.size.width + (pointer.clientX - initialPointer.x) / initialZoom),
        ),
        height: Math.max(
          minSize.height,
          Math.round(node.size.height + (pointer.clientY - initialPointer.y) / initialZoom),
        ),
      };
      if (size.width === current.size.width && size.height === current.size.height) return;
      store.getState().setNodes(store.getState().nodes.map((candidate) => (
        candidate.id === node.id ? { ...candidate, size } : candidate
      )), false);
    };
    const resizeFrames = createAnimationFrameCoalescer(applyResizeFrame);

    const finish = (pointer: PointerEvent) => {
      resizeFrames.flush(pointer);
      const currentState = store.getState();
      if (currentState.nodes === initialNodes) return;
      store.setState({
        future: [],
        historyGroup: null,
        past: [
          ...currentState.past.slice(-99),
          {
            metadata: initialMetadata,
            nodes: initialNodes,
            edges: initialEdges,
            documents: initialDocuments,
            activeDocumentId: initialActiveDocumentId,
          },
        ],
      });
    };

    bindPointerGesture({
      move: resizeFrames.schedule,
      finish,
      cancel: () => {
        resizeFrames.cancel();
        store.getState().setNodes(initialNodes, false);
      },
    });
  }, [minSize.height, minSize.width, node, store]);
}
