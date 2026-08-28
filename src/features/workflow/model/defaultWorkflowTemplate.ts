import type {
  JsonObject,
  UiExecutionPolicy,
  ValueExpr,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import type { NormalizedRect } from './visual';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';
import {
  WECHAT_HEADER_REGION,
  WECHAT_MESSAGE_REGION,
  WECHAT_SEARCH_RESULTS_REGION,
  createWechatInputExecutionPolicy,
  createWechatPressKeyOperation,
  createWechatTypeTextOperation,
  createWechatVisualClickOperation,
  createWechatVisualExecutionPolicy,
  createWechatVisualGetTextOperation,
} from './wechatTemplateParts';

/** 获取或启动微信并提供稳定 AppSession 的节点。 */
export const WECHAT_APPLICATION_NODE_ID = 'wechat_application_1';

/** 打开微信全局搜索框的组合键节点。 */
export const WECHAT_OPEN_SEARCH_NODE_ID = 'wechat_open_search_1';

/** 确认搜索界面已经出现的视觉读取节点。 */
export const WECHAT_VERIFY_SEARCH_NODE_ID = 'wechat_verify_search_1';

/** 搜索框内选中已有文字的组合键节点。 */
export const WECHAT_SELECT_SEARCH_TEXT_NODE_ID = 'wechat_select_search_text_1';

/** 输入目标群名称的节点。 */
export const WECHAT_TYPE_GROUP_NAME_NODE_ID = 'wechat_type_group_name_1';

/** 视觉点击搜索结果的节点。 */
export const WECHAT_CLICK_GROUP_NODE_ID = 'wechat_click_group_1';

/** 输入测试消息的节点。 */
export const WECHAT_TYPE_MESSAGE_NODE_ID = 'wechat_type_message_1';

/** 发送消息的节点。 */
export const WECHAT_SEND_MESSAGE_NODE_ID = 'wechat_send_message_1';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '搜索微信群并发送测试消息';

/** 微信流程不依赖持久化变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 群名称和消息内容作为每次运行输入，避免写死在节点实现里。 */
export const DEFAULT_WORKFLOW_INPUTS = [
  { key: 'group_name', value_type: 'text' },
  { key: 'message', value_type: 'text' },
] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 当前验证场景的中性预填运行输入；用户可在运行前修改。 */
export const DEFAULT_RUN_INPUT_VALUES = {
  group_name: 'ArgusFlow 测试群',
  message: 'ArgusFlow 自动化测试消息',
} as const satisfies JsonObject;

/** AttachOrStart 可能启动微信，因此只声明应用启动能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  allow: ['process.application.launch'],
} as const satisfies WorkflowPermissions;

/** 默认选中应用节点，便于先核对 EXE 和窗口匹配条件。 */
export const DEFAULT_SELECTED_NODE_ID = WECHAT_APPLICATION_NODE_ID;

/** 默认微信流程按“准备、搜索、定位、发送、确认”串联完整闭环。 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', outputBindings: {}, runState: 'idle' },
  },
  {
    id: WECHAT_APPLICATION_NODE_ID,
    kind: 'application',
    position: { x: 188, y: 104 },
    size: { ...WORKFLOW_NODE_SIZES.application },
    data: {
      kind: 'application',
      label: '打开微信',
      outputBindings: {},
      spec: {
        executable_path: 'C:\\Program Files\\Tencent\\Weixin\\Weixin.exe',
        arguments: [],
        window_title: { type: 'equal', value: '微信' },
        acquire_policy: 'attach_or_start',
        launch_timeout_ms: 15_000,
        cleanup_policy: 'leave_running',
        activation_policy: 'required',
      },
      runState: 'idle',
    },
  },
  createUiNode(
    WECHAT_OPEN_SEARCH_NODE_ID,
    '打开搜索',
    { x: 402, y: 104 },
    createWechatPressKeyOperation(
      WECHAT_APPLICATION_NODE_ID,
      { type: 'character', value: 'f' },
      ['control'],
    ),
  ),
  createVisualGetTextNode(
    WECHAT_VERIFY_SEARCH_NODE_ID,
    '确认搜索界面',
    { x: 610, y: 104 },
    literalText('搜索'),
    false,
    WECHAT_SEARCH_RESULTS_REGION,
  ),
  createUiNode(
    WECHAT_SELECT_SEARCH_TEXT_NODE_ID,
    '选中搜索文字',
    { x: 788, y: 104 },
    createWechatPressKeyOperation(
      WECHAT_APPLICATION_NODE_ID,
      { type: 'character', value: 'a' },
      ['control'],
    ),
  ),
  createUiNode(
    WECHAT_TYPE_GROUP_NAME_NODE_ID,
    '输入群名称',
    { x: 1002, y: 104 },
    createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, 'group_name'),
  ),
  createVisualGetTextNode(
    'wechat_find_group_1',
    '确认群搜索结果',
    { x: 1002, y: 220 },
    workflowInputText('group_name'),
    true,
    WECHAT_SEARCH_RESULTS_REGION,
  ),
  createUiNode(
    WECHAT_CLICK_GROUP_NODE_ID,
    '打开群聊',
    { x: 788, y: 220 },
    createWechatVisualClickOperation(
      WECHAT_APPLICATION_NODE_ID,
      workflowInputText('group_name'),
      true,
      WECHAT_SEARCH_RESULTS_REGION,
    ),
    createWechatVisualExecutionPolicy(),
  ),
  createVisualGetTextNode(
    'wechat_verify_header_1',
    '确认群聊标题',
    { x: 610, y: 220 },
    workflowInputText('group_name'),
    true,
    WECHAT_HEADER_REGION,
  ),
  createUiNode(
    WECHAT_TYPE_MESSAGE_NODE_ID,
    '输入测试消息',
    { x: 402, y: 220 },
    createWechatTypeTextOperation(WECHAT_APPLICATION_NODE_ID, 'message'),
  ),
  createUiNode(
    WECHAT_SEND_MESSAGE_NODE_ID,
    '发送消息',
    { x: 188, y: 220 },
    createWechatPressKeyOperation(WECHAT_APPLICATION_NODE_ID, { type: 'enter' }, []),
  ),
  createVisualGetTextNode(
    'wechat_verify_message_1',
    '确认消息已发送',
    { x: 28, y: 336 },
    workflowInputText('message'),
    true,
    WECHAT_MESSAGE_REGION,
  ),
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 240, y: 336 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', outputBindings: {}, runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板的边按视觉验证门逐段串联，删除固定等待节点。 */
export const DEFAULT_EDGES = [
  createDefaultEdge('edge_start_application', 'start_1', WECHAT_APPLICATION_NODE_ID),
  createDefaultEdge('edge_application_search', WECHAT_APPLICATION_NODE_ID, WECHAT_OPEN_SEARCH_NODE_ID),
  createDefaultEdge('edge_search_ready', WECHAT_OPEN_SEARCH_NODE_ID, WECHAT_VERIFY_SEARCH_NODE_ID),
  createDefaultEdge('edge_ready_select', WECHAT_VERIFY_SEARCH_NODE_ID, WECHAT_SELECT_SEARCH_TEXT_NODE_ID),
  createDefaultEdge('edge_select_group_name', WECHAT_SELECT_SEARCH_TEXT_NODE_ID, WECHAT_TYPE_GROUP_NAME_NODE_ID),
  createDefaultEdge(
    'edge_group_name_find',
    WECHAT_TYPE_GROUP_NAME_NODE_ID,
    'wechat_find_group_1',
    'bottom',
    'top',
  ),
  createDefaultEdge('edge_find_click', 'wechat_find_group_1', WECHAT_CLICK_GROUP_NODE_ID, 'left'),
  createDefaultEdge('edge_click_header', WECHAT_CLICK_GROUP_NODE_ID, 'wechat_verify_header_1', 'left'),
  createDefaultEdge('edge_header_message', 'wechat_verify_header_1', WECHAT_TYPE_MESSAGE_NODE_ID, 'left'),
  createDefaultEdge('edge_message_send', WECHAT_TYPE_MESSAGE_NODE_ID, WECHAT_SEND_MESSAGE_NODE_ID, 'left'),
  createDefaultEdge(
    'edge_send_verify',
    WECHAT_SEND_MESSAGE_NODE_ID,
    'wechat_verify_message_1',
    'bottom',
    'top',
  ),
  createDefaultEdge('edge_verify_end', 'wechat_verify_message_1', 'end_1', 'right', 'left'),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

type UiNodeOperation = Extract<WorkflowCanvasNode['data'], { kind: 'ui' }>['operation'];

/** 创建一个使用共享 UI operation helper 的画布 UI 节点。 */
function createUiNode(
  id: string,
  label: string,
  position: WorkflowCanvasNode['position'],
  operation: UiNodeOperation,
  execution: UiExecutionPolicy = createWechatInputExecutionPolicy(),
): WorkflowCanvasNode {
  return {
    id,
    kind: 'ui',
    position,
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label,
      outputBindings: {},
      operation,
      execution,
      runState: 'idle',
    },
  };
}

/** 创建视觉读取节点，输出文本事实供校验与后续调试使用。 */
function createVisualGetTextNode(
  id: string,
  label: string,
  position: WorkflowCanvasNode['position'],
  text: ValueExpr,
  exact: boolean,
  region: NormalizedRect,
): WorkflowCanvasNode {
  return createUiNode(
    id,
    label,
    position,
    createWechatVisualGetTextOperation(
      WECHAT_APPLICATION_NODE_ID,
      text,
      exact,
      region,
    ),
    createWechatVisualExecutionPolicy(),
  );
}

/** 创建字符串字面量值表达式。 */
function literalText(value: string): ValueExpr {
  return { type: 'literal', value };
}

/** 创建读取本次运行输入的字符串表达式。 */
function workflowInputText(key: 'group_name' | 'message'): ValueExpr {
  return {
    type: 'ref',
    source: { type: 'workflow_input', key },
    pointer: '',
  };
}

/** 创建默认示例中的无分支连线。 */
function createDefaultEdge(
  id: string,
  sourceNodeId: string,
  targetNodeId: string,
  sourceSide: WorkflowCanvasEdge['source']['side'] = 'right',
  targetSide: WorkflowCanvasEdge['target']['side'] = 'left',
): WorkflowCanvasEdge {
  return {
    id,
    source: { nodeId: sourceNodeId, side: sourceSide },
    target: { nodeId: targetNodeId, side: targetSide },
    data: { branch: null },
  };
}
