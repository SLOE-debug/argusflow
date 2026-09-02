import type {
  UiExecutionPolicy,
  UiOperation,
  WorkflowResourceCatalog,
} from '../../../../features/workflow';
import { buildActionInspectorViewModel } from '../../../../features/workflow';
import { ActionSection } from '../action/ActionSection';
import { RecoverySection } from '../action/RecoverySection';
import { TargetSection } from '../action/TargetSection';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';

type ActionNodeFieldsProps = Readonly<{
  /** 当前 UI 节点的稳定标识，用于隔离 AQL 文档。 */
  nodeId: string;
  /** 当前 UI 节点的完整语义操作契约。 */
  operation: UiOperation;
  /** 与目标定位语义分离的节点执行预算。 */
  execution: UiExecutionPolicy;
  /** 当前节点可见的应用和浏览器资源目录。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 当前节点是否存在配置错误。 */
  invalid?: boolean;
  /** 写回字段完整的新操作。 */
  onChange: (operation: UiOperation) => void;
  /** 写回字段完整的新执行策略。 */
  onExecutionChange: (execution: UiExecutionPolicy) => void;
  /** 请求 Workspace 打开一个结构化文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

/** 以任务意图顺序组合动作、目标和恢复三个主面板区块。 */
export function ActionNodeFields({
  nodeId,
  operation,
  execution,
  resourceCatalog,
  invalid = false,
  onChange,
  onExecutionChange,
  onOpenEditor,
}: ActionNodeFieldsProps) {
  const viewModel = buildActionInspectorViewModel(
    operation,
    execution,
    resourceCatalog,
    invalid,
  );
  return (
    <>
      <ActionSection
        operation={operation}
        execution={execution}
        onOperationChange={onChange}
        onExecutionChange={onExecutionChange}
      />
      <TargetSection
        nodeId={nodeId}
        operation={operation}
        execution={execution}
        viewModel={viewModel}
        resourceCatalog={resourceCatalog}
        onOperationChange={onChange}
        onExecutionChange={onExecutionChange}
        onOpenEditor={onOpenEditor}
      />
      <RecoverySection
        locatorKind={operation.target.locator.type}
        execution={execution}
        onChange={onExecutionChange}
      />
    </>
  );
}
