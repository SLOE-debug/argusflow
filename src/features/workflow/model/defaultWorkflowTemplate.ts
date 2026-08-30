import type {
  JsonObject,
  ValueExpr,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  asObject,
  asApplicationSpec,
  asUiExecutionPolicy,
  rewriteCanonicalOperationReferences,
} from './canonicalPayload';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';
import { createWechatMessageDefinition } from '../components/builtin/wechatMessage';

/** 获取或启动微信并提供稳定 AppSession 的节点。 */
export const WECHAT_APPLICATION_NODE_ID = 'wechat_application_1';

/** 打开微信全局搜索框的组合键节点。 */
export const WECHAT_OPEN_SEARCH_NODE_ID = 'wechat_open_search_1';

/** 确认搜索界面已经出现的视觉读取节点。 */
export const WECHAT_VERIFY_SEARCH_NODE_ID = 'wechat_verify_search_1';

/** 搜索框内选中已有文字的组合键节点。 */
export const WECHAT_SELECT_SEARCH_TEXT_NODE_ID = 'wechat_select_search_text_1';

/** 输入目标联系人名称的节点。 */
export const WECHAT_TYPE_CONTACT_NAME_NODE_ID = 'wechat_type_contact_name_1';

/** 视觉点击搜索结果的节点。 */
export const WECHAT_CLICK_CONTACT_NODE_ID = 'wechat_click_contact_1';

/** 输入测试消息的节点。 */
export const WECHAT_TYPE_MESSAGE_NODE_ID = 'wechat_type_message_1';

/** 发送消息的节点。 */
export const WECHAT_SEND_MESSAGE_NODE_ID = 'wechat_send_message_1';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '搜索微信联系人并发送测试消息';

/** 微信流程不依赖持久化变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 联系人名称和消息内容作为每次运行输入，避免写死在节点实现里。 */
export const DEFAULT_WORKFLOW_INPUTS = [
  { key: 'contact_name', value_type: 'text' },
  { key: 'message', value_type: 'text' },
] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 当前验证场景的中性预填运行输入；用户可在运行前修改。 */
export const DEFAULT_RUN_INPUT_VALUES = {
  contact_name: '崽崽',
  message: '今日天气',
} as const satisfies JsonObject;

/** AttachOrStart 可能启动微信，因此只声明应用启动能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  allow: ['process.application.launch'],
} as const satisfies WorkflowPermissions;

/** 默认选中应用节点，便于先核对 EXE 和窗口匹配条件。 */
export const DEFAULT_SELECTED_NODE_ID = WECHAT_APPLICATION_NODE_ID;

/** 默认模板直接由 canonical 微信组件图展开，避免维护第二套节点拓扑。 */
export const DEFAULT_NODES = createDefaultWechatNodes();

/** 默认模板的边由 canonical 微信组件图转换，并保留画布端口布局。 */
export const DEFAULT_EDGES = createDefaultWechatEdges();

/** 把 canonical 组件节点转换成主画布节点，转换只负责展示布局和类型适配。 */
function createDefaultWechatNodes(): ReadonlyArray<WorkflowCanvasNode> {
  const definition = createWechatMessageDefinition();
  return definition.nodes.map((node) => {
    const id = canvasNodeId(node.id);
    const position = canvasPosition(node.id);
    const outputBindings = node.output_bindings;
    if (node.type_id === 'argus.start') {
      return {
        id,
        kind: 'start',
        position,
        size: { ...WORKFLOW_NODE_SIZES.start },
        data: { kind: 'start', label: '开始', outputBindings, runState: 'idle' },
      };
    }
    if (node.type_id === 'argus.end') {
      return {
        id,
        kind: 'end',
        position,
        size: { ...WORKFLOW_NODE_SIZES.end },
        data: { kind: 'end', label: '结束', outputBindings, runState: 'idle' },
      };
    }
    if (node.type_id === 'argus.application') {
      const payload = asObject(node.payload);
      return {
        id,
        kind: 'application',
        position,
        size: { ...WORKFLOW_NODE_SIZES.application },
        data: {
          kind: 'application',
          label: '打开微信',
          outputBindings,
          spec: asApplicationSpec(payload.spec),
          runState: 'idle',
        },
      };
    }
    if (node.type_id === 'argus.ui') {
      const payload = asObject(node.payload);
      return {
        id,
        kind: 'ui',
        position,
        size: { ...WORKFLOW_NODE_SIZES.ui },
        data: {
          kind: 'ui',
          label: canvasNodeLabel(node.id),
          outputBindings,
          operation: rewriteCanonicalOperationReferences(
            payload.operation,
            WECHAT_APPLICATION_NODE_ID,
          ),
          execution: asUiExecutionPolicy(payload.execution),
          runState: 'idle',
        },
      };
    }
    return unreachableCanonicalNode(node.type_id);
  });
}

/** 把 canonical 组件边转换为主画布边，并保持默认示例的可读路由方向。 */
function createDefaultWechatEdges(): ReadonlyArray<WorkflowCanvasEdge> {
  const definition = createWechatMessageDefinition();
  return definition.edges.map((edge) => {
    const source = canvasNodeId(edge.source);
    const target = canvasNodeId(edge.target);
    const port = defaultPortFor(edge.source, edge.target);
    return createDefaultEdge(port.id, source, target, port.sourceSide, port.targetSide);
  });
}

/** 画布节点 ID 映射；组件内部 ID 保持独立，主画布 ID 保留现有稳定名称。 */
function canvasNodeId(nodeId: string): string {
  const stableIds: Readonly<Record<string, string>> = {
    entry: 'start_1',
    wechat_application: WECHAT_APPLICATION_NODE_ID,
    open_search: WECHAT_OPEN_SEARCH_NODE_ID,
    verify_search: WECHAT_VERIFY_SEARCH_NODE_ID,
    select_search: WECHAT_SELECT_SEARCH_TEXT_NODE_ID,
    type_contact: WECHAT_TYPE_CONTACT_NAME_NODE_ID,
    click_contact: WECHAT_CLICK_CONTACT_NODE_ID,
    type_message: WECHAT_TYPE_MESSAGE_NODE_ID,
    send_message: WECHAT_SEND_MESSAGE_NODE_ID,
    exit: 'end_1',
  };
  const stableId = stableIds[nodeId];
  if (stableId) return stableId;
  return `${nodeId.startsWith('wechat_') ? nodeId : `wechat_${nodeId}`}_1`;
}

/** 默认示例使用的紧凑二维布局，拓扑本身不在这里定义。 */
function canvasPosition(nodeId: string): { x: number; y: number } {
  const positions: Readonly<Record<string, { x: number; y: number }>> = {
    entry: { x: 28, y: 104 },
    wechat_application: { x: 188, y: 104 },
    open_search: { x: 402, y: 104 },
    verify_search: { x: 610, y: 104 },
    select_search: { x: 820, y: 104 },
    type_contact: { x: 1024, y: 104 },
    click_contact: { x: 820, y: 220 },
    verify_header: { x: 616, y: 220 },
    type_message: { x: 402, y: 220 },
    send_message: { x: 188, y: 220 },
    exit: { x: 240, y: 336 },
  };
  return positions[nodeId] ?? { x: 28, y: 104 };
}

/** canonical 节点在主画布上的用户可读标题。 */
function canvasNodeLabel(nodeId: string): string {
  const labels: Readonly<Record<string, string>> = {
    open_search: '打开搜索',
    verify_search: '确认搜索界面',
    select_search: '选中搜索文字',
    type_contact: '输入联系人名称',
    click_contact: '打开联系人会话',
    verify_header: '确认联系人标题',
    type_message: '输入测试消息',
    send_message: '发送消息',
  };
  return labels[nodeId] ?? nodeId;
}

/** 默认画布的稳定边 ID 与端口方向映射。 */
type DefaultPort = {
  id: string;
  sourceSide: WorkflowCanvasEdge['source']['side'];
  targetSide: WorkflowCanvasEdge['target']['side'];
};

function defaultPortFor(
  source: string,
  target: string,
): DefaultPort {
  const key = `${source}:${target}`;
  const ports: Readonly<Record<string, DefaultPort>> = {
    'entry:wechat_application': { id: 'edge_start_application', sourceSide: 'right', targetSide: 'left' },
    'wechat_application:open_search': { id: 'edge_application_search', sourceSide: 'right', targetSide: 'left' },
    'open_search:verify_search': { id: 'edge_search_ready', sourceSide: 'right', targetSide: 'left' },
    'verify_search:select_search': { id: 'edge_ready_select', sourceSide: 'right', targetSide: 'left' },
    'select_search:type_contact': { id: 'edge_select_contact_name', sourceSide: 'right', targetSide: 'left' },
    'type_contact:click_contact': { id: 'edge_contact_name_click', sourceSide: 'bottom', targetSide: 'right' },
    'click_contact:verify_header': { id: 'edge_click_header', sourceSide: 'left', targetSide: 'right' },
    'verify_header:type_message': { id: 'edge_header_message', sourceSide: 'left', targetSide: 'right' },
    'type_message:send_message': { id: 'edge_message_send', sourceSide: 'left', targetSide: 'right' },
    'send_message:exit': { id: 'edge_send_end', sourceSide: 'bottom', targetSide: 'top' },
  };
  return ports[key] ?? {
    id: `edge_${source}_${target}`,
    sourceSide: 'right',
    targetSide: 'left',
  };
}

/** 创建默认示例中的无分支连线。 */
function createDefaultEdge(
  id: string,
  sourceNodeId: string,
  targetNodeId: string,
  sourceSide: WorkflowCanvasEdge['source']['side'],
  targetSide: WorkflowCanvasEdge['target']['side'],
): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: sourceNodeId, side: sourceSide },
    target: { nodeId: targetNodeId, side: targetSide },
    data: { branch: null },
  };
}

/** canonical factory 发生未预期类型时中止初始化，避免静默生成错误工作流。 */
function unreachableCanonicalNode(typeId: string): never {
  throw new Error(`unsupported canonical WeChat node type: ${typeId}`);
}

/** 创建从流程输入读取字符串的表达式，供外部模板扩展复用。 */
export function workflowInputText(key: 'contact_name' | 'message'): ValueExpr {
  return {
    type: 'ref',
    source: { type: 'workflow_input', key },
    pointer: '',
  };
}
