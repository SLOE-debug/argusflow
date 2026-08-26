/** 节点库与通用画布之间传递节点注册键的原生拖放数据类型。 */
export const FLOW_NODE_KIND_DRAG_TYPE = 'application/x-flow-node-kind';

/** WebView 原生拖放会稳定暴露的文本类型，用于让画布正确接受拖入手势。 */
const FLOW_NODE_KIND_DRAG_FALLBACK_TYPE = 'text/plain';

/** 文本回退负载的应用前缀，避免普通文本恰好等于节点注册键。 */
const FLOW_NODE_KIND_DRAG_FALLBACK_PREFIX = 'argusflow-node:';

/** 同时写入领域类型和浏览器文本类型，避免拖入画布时显示禁止光标。 */
export function writeFlowNodeKindDragData(
  dataTransfer: DataTransfer,
  nodeKind: string,
): void {
  dataTransfer.setData(FLOW_NODE_KIND_DRAG_TYPE, nodeKind);
  dataTransfer.setData(
    FLOW_NODE_KIND_DRAG_FALLBACK_TYPE,
    `${FLOW_NODE_KIND_DRAG_FALLBACK_PREFIX}${nodeKind}`,
  );
}

/** 优先读取领域类型，并在 WebView 未保留自定义类型时读取文本负载。 */
export function readFlowNodeKindDragData(dataTransfer: DataTransfer): string {
  const typedNodeKind = dataTransfer.getData(FLOW_NODE_KIND_DRAG_TYPE);
  if (typedNodeKind) return typedNodeKind;

  const fallbackNodeKind = dataTransfer.getData(FLOW_NODE_KIND_DRAG_FALLBACK_TYPE);
  return fallbackNodeKind.startsWith(FLOW_NODE_KIND_DRAG_FALLBACK_PREFIX)
    ? fallbackNodeKind.slice(FLOW_NODE_KIND_DRAG_FALLBACK_PREFIX.length)
    : '';
}

