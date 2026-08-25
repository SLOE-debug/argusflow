import type { JsonObject } from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '启动或唤醒 Notepad++ 并打开帮助菜单';

/** 默认模板不制造未被动作消费的演示变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 默认选中真实可配置的 UI 自动化节点。 */
export const DEFAULT_SELECTED_NODE_ID = 'open_notepadpp_search_1';

/**
 * 可直接执行的线性示例：复用并恢复 Notepad++，或在不存在时启动它，再通过 UIA 打开帮助菜单。
 */
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
    kind: 'action',
    position: { x: 188, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.action },
    data: {
      kind: 'action',
      label: '唤醒 Notepad++',
      action: {
        type: 'click',
        target: {
          locator: {
            type: 'application_query',
            application: {
              executable_path: 'C:\\Program Files\\Notepad++\\notepad++.exe',
              arguments: [],
              window_title: { type: 'contains', value: 'Notepad++' },
              launch_timeout_ms: 10_000,
            },
            query: {
              language_version: 1,
              source: 'first(window(name contains "Notepad++") >> menu_item(name = "?"))',
            },
          },
          backend_preference: 'windows_uia',
        },
      },
      runState: 'idle',
    },
  },
  {
    id: 'log_result_1',
    kind: 'log',
    position: { x: 394, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.log },
    data: {
      kind: 'log',
      label: '记录结果',
      message: '已唤醒 Notepad++ 并通过 UIA 打开帮助菜单',
      runState: 'idle',
    },
  },
  {
    id: 'end_1',
    kind: 'end',
    position: { x: 578, y: 148 },
    size: { ...WORKFLOW_NODE_SIZES.end },
    data: { kind: 'end', label: '结束', runState: 'idle' },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasNode>;

/** 默认模板只包含一条可追踪的实际执行路径。 */
export const DEFAULT_EDGES = [
  {
    id: 'edge_start_open_notepadpp',
    source: { nodeId: 'start_1', side: 'right' },
    target: { nodeId: DEFAULT_SELECTED_NODE_ID, side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_open_notepadpp_log',
    source: { nodeId: DEFAULT_SELECTED_NODE_ID, side: 'right' },
    target: { nodeId: 'log_result_1', side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_log_end',
    source: { nodeId: 'log_result_1', side: 'right' },
    target: { nodeId: 'end_1', side: 'left' },
    data: { branch: null },
  },
] as const satisfies ReadonlyArray<WorkflowCanvasEdge>;
