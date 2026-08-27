import type { FlowAnchorSide } from '../../../flow';
import type { WorkflowCanvasNode } from '../model/workflowModel';

/** 返回起点锚点的对侧，作为新节点的默认入口。 */
export function oppositeAnchorSide(side: FlowAnchorSide): FlowAnchorSide {
  switch (side) {
    case 'top':
      return 'bottom';
    case 'right':
      return 'left';
    case 'bottom':
      return 'top';
    case 'left':
      return 'right';
  }
}

/** 为直接连接在资源节点后的操作写入显式逻辑资源引用。 */
export function bindConnectedResourceScope(
  node: WorkflowCanvasNode,
  source: WorkflowCanvasNode | undefined,
): WorkflowCanvasNode {
  if (source?.data.kind === 'browser' && node.data.kind === 'navigate') {
    return {
      ...node,
      data: {
        ...node.data,
        operation: {
          ...node.data.operation,
          browser: {
            producer_node_id: source.id,
            output_name: 'session',
          },
        },
      },
    };
  }
  if (node.data.kind !== 'ui'
    || (source?.data.kind !== 'application' && source?.data.kind !== 'browser')) return node;
  return {
    ...node,
    data: {
      ...node.data,
      operation: {
        ...node.data.operation,
        target: {
          ...node.data.operation.target,
          scope: {
            type: source.data.kind,
            resource: {
              producer_node_id: source.id,
              output_name: 'session',
            },
          },
        },
      },
    },
  };
}
