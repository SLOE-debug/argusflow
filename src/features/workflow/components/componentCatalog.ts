import type {
  ComponentInstance,
  FlowComponentDefinition,
  ValueOutputDescriptor,
} from '../model/contracts';
import { createDefaultBrowserSpec } from '../nodes/workflowBrowser';

/** 官方网页链接采集组件的稳定 ID。 */
export const WEB_LINK_COLLECTOR_COMPONENT_ID = '1a3c4624-90a5-45f0-86e8-ae3d04b817c1';

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
