import type {
  JsonObject,
  WorkflowInputDefinition,
  WorkflowPermissions,
} from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '打开或连接 Notepad++ 并读取窗口文本';

/** 默认模板不制造未被节点消费的演示变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 默认模板不声明运行时输入。 */
export const DEFAULT_WORKFLOW_INPUTS = [] as const satisfies ReadonlyArray<WorkflowInputDefinition>;

/** 默认模板本次运行没有瞬时输入值。 */
export const DEFAULT_RUN_INPUT_VALUES = {} as const satisfies JsonObject;

/** 默认模板只授权示例 Application 节点可能需要的启动能力。 */
export const DEFAULT_WORKFLOW_PERMISSIONS = {
  application_launch: true,
  direct_command: false,
  powershell: false,
  cmd: false,
} as const satisfies WorkflowPermissions;

/** 默认选中应用资源节点，突出新的资源生命周期模型。 */
export const DEFAULT_SELECTED_NODE_ID = 'notepadpp_app_1';

/** 可直接执行的 AppSession → GetText → Debug 线性示例。 */
export const DEFAULT_NODES = [
  {
    id: 'start_1',
    kind: 'start',
    position: { x: 28, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.start },
    data: { kind: 'start', label: '开始', runState: 'idle' },
  },
  {
    id: DEFAULT_SELECTED_NODE_ID,
    kind: 'application',
    position: { x: 184, y: 148 },
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
    id: 'read_notepadpp_title_1',
    kind: 'ui',
    position: { x: 398, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.ui },
    data: {
      kind: 'ui',
      label: '读取窗口标题',
      operation: {
        type: 'get_text',
        target: {
          scope: {
            type: 'application',
            resource: {
              producer_node_id: DEFAULT_SELECTED_NODE_ID,
              output_name: 'session',
            },
          },
          locator: {
            type: 'query',
            query: {
              language_version: 1,
              source: 'first(window(name contains "Notepad++"))',
            },
          },
          backend_preference: 'windows_uia',
        },
      },
      runState: 'idle',
    },
  },
  {
    id: 'debug_notepadpp_title_1',
    kind: 'debug',
    position: { x: 602, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.debug },
    data: {
      kind: 'debug',
      label: '输出窗口标题',
      value: {
        type: 'node_output',
        node_id: 'read_notepadpp_title_1',
        output: 'text',
      },
      runState: 'idle',
    },
  },
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 798, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板只包含一条可追踪的实际执行路径。 */
export const DEFAULT_EDGES = [
  {
    id: 'edge_start_application',
    source: { nodeId: 'start_1', side: 'right' },
    target: { nodeId: DEFAULT_SELECTED_NODE_ID, side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_application_read',
    source: { nodeId: DEFAULT_SELECTED_NODE_ID, side: 'right' },
    target: { nodeId: 'read_notepadpp_title_1', side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_read_debug',
    source: { nodeId: 'read_notepadpp_title_1', side: 'right' },
    target: { nodeId: 'debug_notepadpp_title_1', side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_debug_end',
    source: { nodeId: 'debug_notepadpp_title_1', side: 'right' },
    target: { nodeId: 'end_1', side: 'left' },
    data: { branch: null },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;
