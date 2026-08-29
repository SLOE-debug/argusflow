import type {
  ApplicationSpec,
  FlowComponentDefinition,
  JsonValue,
  ValueExpr,
} from '../../model/contracts';
import {
  WECHAT_HEADER_REGION,
  WECHAT_SEARCH_OVERLAY_REGION,
  WECHAT_SEARCH_RESULTS_REGION,
  createWechatInputExecutionPolicy,
  createWechatSendMessageExecutionPolicy,
  createWechatPressKeyOperation,
  createWechatTypeTextOperation,
  createWechatVisualClickOperation,
  createWechatVisualExecutionPolicy,
  createWechatVisualGetTextOperation,
} from '../../model/wechatTemplateParts';

/** 官方发送微信群消息组件的稳定 ID。 */
export const WECHAT_MESSAGE_COMPONENT_ID = '4d9c8f1e-3e5b-4e7a-9c4d-2a7d0b6f51c8';

/** 创建微信桌面消息组件的唯一 canonical 节点图。 */
export function createWechatMessageDefinition(): FlowComponentDefinition {
  const applicationNodeId = 'wechat_application';
  const visualExecution = createWechatVisualExecutionPolicy();
  return {
    schema_version: 1,
    id: WECHAT_MESSAGE_COMPONENT_ID,
    version: '1.2.0',
    name: '发送微信群消息',
    inputs: [
      { key: 'group_name', value_type: 'text' },
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
        operation: createWechatVisualGetTextOperation(
          applicationNodeId,
          literalText('网络结果'),
          false,
          WECHAT_SEARCH_OVERLAY_REGION,
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
      componentNode('type_group', 1_060, 'argus.ui', 3, {
        operation: createWechatTypeTextOperation(applicationNodeId, 'group_name'),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('find_group', 1_280, 'argus.ui', 3, {
        operation: createWechatVisualGetTextOperation(
          applicationNodeId,
          workflowInputText('group_name'),
          true,
          WECHAT_SEARCH_RESULTS_REGION,
        ),
        execution: visualExecution,
      }),
      componentNode('click_group', 1_500, 'argus.ui', 3, {
        operation: createWechatVisualClickOperation(
          applicationNodeId,
          workflowInputText('group_name'),
          true,
          WECHAT_SEARCH_RESULTS_REGION,
        ),
        execution: visualExecution,
      }),
      componentNode('verify_header', 1_720, 'argus.ui', 3, {
        operation: createWechatVisualGetTextOperation(
          applicationNodeId,
          workflowInputText('group_name'),
          true,
          WECHAT_HEADER_REGION,
        ),
        execution: visualExecution,
      }),
      componentNode('type_message', 1_940, 'argus.ui', 3, {
        operation: createWechatTypeTextOperation(applicationNodeId, 'message'),
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode('send_message', 2_160, 'argus.ui', 3, {
        operation: createWechatPressKeyOperation(applicationNodeId, { type: 'enter' }, []),
        execution: createWechatSendMessageExecutionPolicy(),
      }),
      componentNode('exit', 2_380, 'argus.end', 1, {}),
    ],
    edges: [
      componentEdge('entry', applicationNodeId),
      componentEdge(applicationNodeId, 'open_search'),
      componentEdge('open_search', 'verify_search'),
      componentEdge('verify_search', 'select_search'),
      componentEdge('select_search', 'type_group'),
      componentEdge('type_group', 'find_group'),
      componentEdge('find_group', 'click_group'),
      componentEdge('click_group', 'verify_header'),
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

/** 创建组件内部的字符串字面量值表达式。 */
function literalText(value: string): ValueExpr {
  return { type: 'literal', value };
}

/** 创建组件输入引用，展开时由 Runtime 重写为实例输入。 */
function workflowInputText(key: 'group_name' | 'message'): ValueExpr {
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
