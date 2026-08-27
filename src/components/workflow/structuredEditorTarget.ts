import type { ValueExprLocation } from '../../features/workflow/workflowValueExpressions';

/** 工作区一次只打开一个结构化文档，目标由文档类别与所属节点共同标识。 */
export type StructuredEditorTarget =
  | {
      /** AQL 查询规则文档。 */
      readonly type: 'aql';
      /** 文档所属工作流节点 ID。 */
      readonly nodeId: string;
    }
  | {
      /** PowerShell 或 CMD 固定文本脚本文档。 */
      readonly type: 'command_script';
      /** 文档所属工作流节点 ID。 */
      readonly nodeId: string;
    }
  | {
      /** Runtime Value Plane 的受限 Rhai 表达式。 */
      readonly type: 'expression';
      /** 文档所属工作流节点 ID。 */
      readonly nodeId: string;
      /** 节点内具体 ValueExpr 字段的稳定路径。 */
      readonly location: ValueExprLocation;
    };

/** 结构化编辑器在中央工作区中的两种非模态布局。 */
export type WorkspaceEditorMode = 'docked' | 'maximized';

/** 不进入 Workflow JSON 的纯界面编辑器状态。 */
export type WorkspaceEditorState = Readonly<{
  /** 当前打开的文档；null 表示 Dock 只展示工具页签。 */
  target: StructuredEditorTarget | null;
  /** 文档编辑器在中央区域中的当前布局。 */
  mode: WorkspaceEditorMode;
  /** docked 布局下用户设置的像素高度。 */
  dockHeight: number;
}>;
