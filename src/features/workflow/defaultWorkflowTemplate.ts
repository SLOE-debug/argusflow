import type { JsonObject } from './contracts';
import {
  WORKFLOW_NODE_SIZES,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
} from './workflowModel';

/** 默认模板的工作流名称。 */
export const DEFAULT_WORKFLOW_NAME = '向已打开的记事本填写文本';

/** 默认模板不制造未被动作消费的演示变量。 */
export const DEFAULT_WORKFLOW_VARIABLES = {} as const satisfies JsonObject;

/** 默认选中真实可配置的 UI 自动化节点。 */
export const DEFAULT_SELECTED_NODE_ID = 'fill_notepad_1';

/**
 * 可直接理解和继续配置的线性示例：用户先打开记事本，流程定位其文档区域并填写文本。
 *
 * 模板不包含无业务来源的条件分支，也不假装具备尚未实现的“启动应用”动作。
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
      label: '填写记事本',
      action: {
        type: 'set_value',
        target: {
          locator: {
            type: 'query',
            query: {
              language_version: 1,
              source: 'first(window(name contains "记事本") >> document())',
            },
          },
          backend_preference: 'windows_uia',
        },
        value: '你好，这段文字由 ArgusFlow 自动填写。',
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
      message: '已完成记事本内容填写',
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
    id: 'edge_start_fill',
    source: { nodeId: 'start_1', side: 'right' },
    target: { nodeId: DEFAULT_SELECTED_NODE_ID, side: 'left' },
    data: { branch: null },
  },
  {
    id: 'edge_fill_log',
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
