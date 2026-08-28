import type {
  ApplicationSpec,
  ComponentInstance,
  FlowComponentDefinition,
  ValueExpr,
  ValueOutputDescriptor,
} from '../model/contracts';
import { createDefaultBrowserSpec } from '../nodes/workflowBrowser';
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
} from '../model/wechatTemplateParts';

/** 官方网页链接采集组件的稳定 ID。 */
export const WEB_LINK_COLLECTOR_COMPONENT_ID = '1a3c4624-90a5-45f0-86e8-ae3d04b817c1';

/** 官方发送微信群消息组件的稳定 ID。 */
export const WECHAT_MESSAGE_COMPONENT_ID = '4d9c8f1e-3e5b-4e7a-9c4d-2a7d0b6f51c8';

/** Studio 可创建的版本锁定组件目录项。 */
export type FlowComponentCatalogItem = Readonly<{
  title: string;
  description: string;
  definition: FlowComponentDefinition;
  defaultInputs: ComponentInstance['inputs'];
  valueOutputs: ReadonlyArray<ValueOutputDescriptor>;
}>;

/** 内置官方组件目录；实例始终引用这里的精确版本。 */
export const FLOW_COMPONENT_CATALOG = [
  {
    title: '网页链接采集',
    description: '打开浏览器并读取网页链接',
    definition: createWebLinkCollectorDefinition(),
    defaultInputs: {
      url: { type: 'literal', value: 'https://www.baidu.com/' },
    },
    valueOutputs: [
      { name: 'items', valueType: 'json', label: '链接列表' },
    ],
  },
  {
    title: '发送微信群消息',
    description: '视觉确认搜索、群聊标题和发送结果',
    definition: createWechatMessageDefinition(),
    defaultInputs: {
      group_name: { type: 'literal', value: 'ArgusFlow 测试群' },
      message: { type: 'literal', value: 'ArgusFlow 自动化测试消息' },
    },
    valueOutputs: [
      { name: 'confirmed', valueType: 'text', label: '发送确认文本' },
    ],
  },
] as const satisfies ReadonlyArray<FlowComponentCatalogItem>;

/** 通过稳定 ID 和精确版本查找目录项。 */
export function findFlowComponent(
  componentId: string,
  componentVersion: string,
  catalog: ReadonlyArray<FlowComponentCatalogItem> = FLOW_COMPONENT_CATALOG,
): FlowComponentCatalogItem | undefined {
  return catalog.find((item) => (
    item.definition.id === componentId
    && item.definition.version === componentVersion
  ));
}

/** 创建由新 Browser v2、Navigate 与通用 Extract 组成的官方示例组件。 */
function createWebLinkCollectorDefinition(): FlowComponentDefinition {
  const browserNodeId = 'acquire_browser';
  const navigateNodeId = 'navigate';
  const extractNodeId = 'extract_links';
  return {
    schema_version: 1,
    id: WEB_LINK_COLLECTOR_COMPONENT_ID,
    version: '1.0.0',
    name: '网页链接采集',
    inputs: [{ key: 'url', value_type: 'text' }],
    outputs: [{
      name: 'items',
      value: {
        type: 'ref',
        source: { type: 'node', node_id: extractNodeId },
        pointer: '/items',
      },
    }],
    nodes: [
      componentNode('entry', 0, 'argus.start', 1, {}),
      componentNode(browserNodeId, 180, 'argus.browser', 2, {
        spec: createDefaultBrowserSpec(),
      }),
      componentNode(navigateNodeId, 400, 'argus.browser.operation', 1, {
        operation: {
          type: 'navigate',
          browser: { producer_node_id: browserNodeId, output_name: 'session' },
          url: {
            type: 'ref',
            source: { type: 'workflow_input', key: 'url' },
            pointer: '',
          },
        },
      }),
      componentNode(extractNodeId, 620, 'argus.ui', 2, {
        operation: {
          type: 'extract',
          target: {
            scope: {
              type: 'browser',
              resource: { producer_node_id: browserNodeId, output_name: 'session' },
            },
            locator: {
              type: 'query',
              query: { language_version: 1, source: 'css("a[href]")' },
            },
            backend_policy: {
              allow: ['browser_cdp'],
              deny: [],
              prefer: ['browser_cdp'],
            },
          },
          cardinality: 'many',
          fields: [
            { name: 'title', source: { type: 'text' } },
            { name: 'url', source: { type: 'attribute', name: 'href' } },
          ],
        },
        execution: {
          target_wait: { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 100 },
        },
      }),
      componentNode('exit', 840, 'argus.end', 1, {}),
    ],
    edges: [
      componentEdge('entry', browserNodeId),
      componentEdge(browserNodeId, navigateNodeId),
      componentEdge(navigateNodeId, extractNodeId),
      componentEdge(extractNodeId, 'exit'),
    ],
    entry_node_id: 'entry',
    exit_node_id: 'exit',
  };
}

/** 创建微信 V2 单入口单出口闭环组件；输入和视觉查询均保持动态 ValueExpr。 */
function createWechatMessageDefinition(): FlowComponentDefinition {
  const applicationNodeId = 'wechat_application';
  const verifyMessageNodeId = 'verify_message';
  const visualExecution = createWechatVisualExecutionPolicy();
  return {
    schema_version: 1,
    id: WECHAT_MESSAGE_COMPONENT_ID,
    version: '1.0.0',
    name: '发送微信群消息',
    inputs: [
      { key: 'group_name', value_type: 'text' },
      { key: 'message', value_type: 'text' },
    ],
    outputs: [{
      name: 'confirmed',
      value: {
        type: 'ref',
        source: { type: 'node', node_id: verifyMessageNodeId },
        pointer: '/text',
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
          literalText('搜索'),
          false,
          WECHAT_SEARCH_RESULTS_REGION,
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
        execution: createWechatInputExecutionPolicy(),
      }),
      componentNode(verifyMessageNodeId, 2_380, 'argus.ui', 3, {
        operation: createWechatVisualGetTextOperation(
          applicationNodeId,
          workflowInputText('message'),
          true,
          WECHAT_MESSAGE_REGION,
        ),
        execution: visualExecution,
      }),
      componentNode('exit', 2_600, 'argus.end', 1, {}),
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
      componentEdge('send_message', verifyMessageNodeId),
      componentEdge(verifyMessageNodeId, 'exit'),
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
  payload: import('../model/contracts').JsonValue,
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
