import type {
  ApplicationSpec,
  FlowComponentDefinition,
  JsonValue,
  ValueExpr,
} from '../../model/contracts';
import {
  createWechatAqlClickOperation,
  createWechatAqlGetTextOperation,
  createWechatInputExecutionPolicy,
  createWechatSendMessageExecutionPolicy,
  createWechatPressKeyOperation,
  createWechatTypeTextOperation,
  createWechatVisualExecutionPolicy,
} from '../../model/wechatTemplateParts';

/** 官方发送微信联系人消息组件的稳定 ID。 */
export const WECHAT_MESSAGE_COMPONENT_ID = '4d9c8f1e-3e5b-4e7a-9c4d-2a7d0b6f51c8';

/** 将“最常使用”作为空间锚点，只选择其下方标题精确匹配的联系人。 */
const WECHAT_CONTACT_RESULT_QUERY =
  'nearest(anchor = text(name = "最常使用"), target = text(name = $contact_name), direction = below, index = 1)';

/** 右上角窗口关闭标记用于排除左侧搜索框和会话列表中的同名文本。 */
const WECHAT_CONTACT_HEADER_QUERY =
  'nearest(anchor = text(name = "X"), target = text(name = $contact_name), direction = left, index = 1)';

/** 创建微信桌面消息组件的唯一 canonical 节点图。 */
export function createWechatMessageDefinition(): FlowComponentDefinition {
  const applicationNodeId = 'wechat_application';
  const visualExecution = createWechatVisualExecutionPolicy();
  return {
    schema_version: 1,
    id: WECHAT_MESSAGE_COMPONENT_ID,
    version: '3.1.0',
    name: '发送微信联系人消息',
    inputs: [
      { key: 'contact_name', value_type: 'text' },
      { key: 'message', value_type: 'text' },
    ],
    outputs: [{
      name: 'confirmed',
      value: {
        type: 'ref',
        source: { type: 'node', node_id: 'send_message' },
        pointer: '/confirmed',
      },
    }],
    nodes: [
      componentNode('entry', 0, 'argus.start', 1, {}),
      componentNode(applicationNodeId, 180, 'argus.application', 1, {
        spec: createWechatApplicationSpec(),
      }),
      componentNode('open_search', 400, 'argus.ui', 3, {
        operation: createWechatPressKeyOperation(
          applicationNodeId,
          { type: 'character', value: 'f' },
          ['control'],
        ),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('verify_search', 620, 'argus.ui', 3, {
        operation: createWechatAqlGetTextOperation(
          applicationNodeId,
          'text(name contains "网络结果")',
        ),
        execution: visualExecution,
      }),
      componentNode('select_search', 840, 'argus.ui', 3, {
        operation: createWechatPressKeyOperation(
          applicationNodeId,
          { type: 'character', value: 'a' },
          ['control'],
        ),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('type_contact', 1_060, 'argus.ui', 3, {
        operation: createWechatTypeTextOperation(applicationNodeId, 'contact_name'),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('click_contact', 1_280, 'argus.ui', 3, {
        operation: createWechatAqlClickOperation(
          applicationNodeId,
          WECHAT_CONTACT_RESULT_QUERY,
          { contact_name: workflowInputText('contact_name') },
        ),
        execution: visualExecution,
      }),
      componentNode('verify_header', 1_500, 'argus.ui', 3, {
        operation: createWechatAqlGetTextOperation(
          applicationNodeId,
          WECHAT_CONTACT_HEADER_QUERY,
          { contact_name: workflowInputText('contact_name') },
        ),
        execution: visualExecution,
      }),
      componentNode('type_message', 1_720, 'argus.ui', 3, {
        operation: createWechatTypeTextOperation(applicationNodeId, 'message'),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('send_message', 1_940, 'argus.ui', 3, {
        operation: createWechatPressKeyOperation(applicationNodeId, { type: 'enter' }, []),
        execution: createWechatSendMessageExecutionPolicy(),
      }),
      componentNode('exit', 2_160, 'argus.end', 1, {}),
    ],
    edges: [
      componentEdge('entry', applicationNodeId),
      componentEdge(applicationNodeId, 'open_search'),
      componentEdge('open_search', 'verify_search'),
      componentEdge('verify_search', 'select_search'),
      componentEdge('select_search', 'type_contact'),
      componentEdge('type_contact', 'click_contact'),
      componentEdge('click_contact', 'verify_header'),
      componentEdge('verify_header', 'type_message'),
      componentEdge('type_message', 'send_message'),
      componentEdge('send_message', 'exit'),
    ],
    entry_node_id: 'entry',
    exit_node_id: 'exit',
  };
}

/** 微信桌面应用的默认 AttachOrStart 资源契约。 */
function createWechatApplicationSpec(): ApplicationSpec {
  return {
    executable_path: 'C:\\Program Files\\Tencent\\Weixin\\Weixin.exe',
    arguments: [],
    window_title: { type: 'equal', value: '微信' },
    acquire_policy: 'attach_or_start',
    launch_timeout_ms: 15_000,
    cleanup_policy: 'leave_running',
    activation_policy: 'required',
  };
}

/** 创建组件输入引用，展开时由 Runtime 重写为实例输入。 */
function workflowInputText(key: 'contact_name' | 'message'): ValueExpr {
  return {
    type: 'ref',
    source: { type: 'workflow_input', key },
    pointer: '',
  };
}

/** 创建组件内部开放节点契约。 */
function componentNode(
  id: string,
  x: number,
  typeId: string,
  version: number,
  payload: JsonValue,
): FlowComponentDefinition['nodes'][number] {
  return {
    id,
    position: { x, y: 100 },
    type_id: typeId,
    version,
    payload,
    output_bindings: {},
  };
}

/** 创建组件内部无分支控制边。 */
function componentEdge(
  source: string,
  target: string,
): FlowComponentDefinition['edges'][number] {
  return {
    id: `edge_${source}_${target}`,
    source,
    target,
    branch: null,
  };
}
