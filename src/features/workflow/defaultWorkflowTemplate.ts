import type {
  AutomationTarget,
  JsonObject,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 默认示例绑定的 Notepad++ Application 节点 ID。 */
const NOTEPADPP_APPLICATION_NODE_ID = 'notepadpp_app_1';

/** 查找词回读节点 ID，用于演示强类型节点输出传递。 */
const READ_SEARCH_VALUE_NODE_ID = 'read_search_value_1';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '用 UIA 驱动 Notepad++ 查找';

/** 默认模板不制造未被节点消费的演示变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 搜索文字作为瞬时输入传给 ValuePattern，不写入工作流定义。 */
export const DEFAULT_WORKFLOW_INPUTS = [
  { key: 'search_text', value_type: 'text' },
] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 默认运行值让示例无需额外配置即可统计当前文档中的 UIA 文本。 */
export const DEFAULT_RUN_INPUT_VALUES = {
  search_text: 'UIA',
} as const satisfies JsonObject;

/** 默认模板只授权示例 Application 节点可能需要的启动能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  application_launch: true,
  direct_command: false,
  powershell: false,
  cmd: false,
} as const satisfies WorkflowPermissions;

/** 默认选中应用资源节点，突出资源生命周期与后续 UIA 作用域。 */
export const DEFAULT_SELECTED_NODE_ID = NOTEPADPP_APPLICATION_NODE_ID;

/**
 * 可直接执行的中文 Notepad++ UIA 示例。
 *
 * 流程只通过中文可访问名称与控件树关系定位目标：展开“搜索”菜单、打开“查找”
 * 对话框、写入并回读运行时搜索词、执行计数，最后取消对话框。
 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', runState: 'idle' },
  },
  {
    id: NOTEPADPP_APPLICATION_NODE_ID,
    kind: 'application',
    position: { x: 182, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.application },
    data: {
      kind: 'application',
      label: '连接 Notepad++',
      spec: {
        executable_path: 'C:\\Program Files\\Notepad++\\notepad++.exe',
        arguments: [],
        window_title: { type: 'contains', value: 'Notepad++' },
        acquire_policy: 'attach_or_start',
        launch_timeout_ms: 10_000,
        cleanup_policy: 'leave_running',
        activation_policy: 'best_effort',
      },
      runState: 'idle',
    },
  },
  {
    id: 'wait_notepadpp_ready_1',
    kind: 'delay',
    position: { x: 390, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label: '等待 UIA 控件就绪',
      milliseconds: 1_000,
      runState: 'idle',
    },
  },
  {
    id: 'open_search_menu_1',
    kind: 'ui',
    position: { x: 562, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '展开“搜索”菜单',
      operation: {
        type: 'click',
        target: createNotepadppUiaTarget('menu_item(name = "搜索(S)")'),
      },
      runState: 'idle',
    },
  },
  {
    id: 'wait_search_menu_1',
    kind: 'delay',
    position: { x: 762, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label: '等待菜单展开',
      milliseconds: 200,
      runState: 'idle',
    },
  },
  {
    id: 'open_find_dialog_1',
    kind: 'ui',
    position: { x: 934, y: 92 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '调用“查找”菜单项',
      operation: {
        type: 'click',
        target: createNotepadppUiaTarget(
          'menu_item(name starts_with "查找(F)...")',
        ),
      },
      runState: 'idle',
    },
  },
  {
    id: 'wait_find_dialog_1',
    kind: 'delay',
    position: { x: 762, y: 226 },
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label: '等待对话框就绪',
      milliseconds: 300,
      runState: 'idle',
    },
  },
  {
    id: 'set_find_value_1',
    kind: 'ui',
    position: { x: 562, y: 226 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '写入运行时搜索词',
      operation: {
        type: 'set_value',
        target: createNotepadppUiaTarget(
          'dialog(name = "查找") >> textbox(name = "查找目标(F) :")',
        ),
        value: { type: 'workflow_input', key: 'search_text' },
      },
      runState: 'idle',
    },
  },
  {
    id: 'count_matches_1',
    kind: 'ui',
    position: { x: 362, y: 226 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '统计当前文档匹配数',
      operation: {
        type: 'click',
        target: createNotepadppUiaTarget(
          'dialog(name = "查找") >> button(name = "计数(T)")',
        ),
      },
      runState: 'idle',
    },
  },
  {
    id: 'wait_match_count_1',
    kind: 'delay',
    position: { x: 170, y: 226 },
    size: { ...WORKFLOW_NODE_SIZES.delay },
    data: {
      kind: 'delay',
      label: '等待计数结果',
      milliseconds: 300,
      runState: 'idle',
    },
  },
  {
    id: READ_SEARCH_VALUE_NODE_ID,
    kind: 'ui',
    position: { x: 170, y: 360 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '回读运行时搜索词',
      operation: {
        type: 'get_value',
        target: createNotepadppUiaTarget(
          'dialog(name = "查找") >> textbox(name = "查找目标(F) :")',
        ),
      },
      runState: 'idle',
    },
  },
  {
    id: 'debug_match_count_1',
    kind: 'debug',
    position: { x: 362, y: 360 },
    size: { ...WORKFLOW_NODE_SIZES.debug },
    data: {
      kind: 'debug',
      label: '输出回读搜索词',
      value: {
        type: 'node_output',
        node_id: READ_SEARCH_VALUE_NODE_ID,
        output: 'value',
      },
      runState: 'idle',
    },
  },
  {
    id: 'close_find_dialog_1',
    kind: 'ui',
    position: { x: 562, y: 360 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '关闭查找对话框',
      operation: {
        type: 'click',
        target: createNotepadppUiaTarget(
          'dialog(name = "查找") >> button(name = "取消")',
        ),
      },
      runState: 'idle',
    },
  },
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 762, y: 360 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板按蛇形布局串联一条可追踪的真实执行路径。 */
export const DEFAULT_EDGES = [
  createDefaultEdge('edge_start_application', 'start_1', NOTEPADPP_APPLICATION_NODE_ID),
  createDefaultEdge(
    'edge_application_ready',
    NOTEPADPP_APPLICATION_NODE_ID,
    'wait_notepadpp_ready_1',
  ),
  createDefaultEdge(
    'edge_ready_find',
    'wait_notepadpp_ready_1',
    'open_search_menu_1',
  ),
  createDefaultEdge(
    'edge_search_menu_wait',
    'open_search_menu_1',
    'wait_search_menu_1',
  ),
  createDefaultEdge(
    'edge_menu_find',
    'wait_search_menu_1',
    'open_find_dialog_1',
  ),
  createDefaultEdge('edge_find_wait', 'open_find_dialog_1', 'wait_find_dialog_1'),
  createDefaultEdge('edge_wait_set', 'wait_find_dialog_1', 'set_find_value_1'),
  createDefaultEdge(
    'edge_set_count',
    'set_find_value_1',
    'count_matches_1',
    'bottom',
    'top',
  ),
  createDefaultEdge(
    'edge_count_wait',
    'count_matches_1',
    'wait_match_count_1',
    'left',
    'right',
  ),
  createDefaultEdge(
    'edge_wait_read',
    'wait_match_count_1',
    READ_SEARCH_VALUE_NODE_ID,
    'left',
    'right',
  ),
  createDefaultEdge(
    'edge_read_debug',
    READ_SEARCH_VALUE_NODE_ID,
    'debug_match_count_1',
    'left',
    'right',
  ),
  createDefaultEdge(
    'edge_debug_close',
    'debug_match_count_1',
    'close_find_dialog_1',
    'bottom',
    'top',
  ),
  createDefaultEdge('edge_close_end', 'close_find_dialog_1', 'end_1'),
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;

/** 为默认节点建立绑定同一 AppSession 且强制走 Windows UIA 的查询目标。 */
function createNotepadppUiaTarget(source: string): AutomationTarget {
  return {
    scope: {
      type: 'application',
      resource: {
        producer_node_id: NOTEPADPP_APPLICATION_NODE_ID,
        output_name: 'session',
      },
    },
    locator: {
      type: 'query',
      query: { language_version: 1, source },
    },
    backend_preference: 'windows_uia',
  };
}

/** 创建默认示例中的无分支连线，并允许蛇形拐点显式指定锚点。 */
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
