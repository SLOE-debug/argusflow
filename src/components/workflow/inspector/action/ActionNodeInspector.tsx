import type {
  WorkflowCanvasNode,
  WorkflowNodeData,
  WorkflowNodeUpdater,
  WorkflowResourceCatalog,
} from '../../../../features/workflow';
import { buildActionInspectorViewModel } from '../../../../features/workflow';
import {
  InspectorDeleteButton,
  InspectorSection,
} from '../InspectorControls';
import { NodeDeveloperSection } from '../NodeDeveloperSection';
import { NodeInspectorHeader } from '../NodeInspectorHeader';
import { NodeOutputSection } from '../NodeOutputSection';
import { ActionNodeFields } from '../node-fields/ActionNodeFields';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';
import { ActionAdvancedSection } from './ActionAdvancedSection';

type UiNodeData = Extract<WorkflowNodeData, { kind: 'ui' }>;

type ActionNodeInspectorProps = Readonly<{
  /** 当前 UI 节点的稳定 ID。 */
  nodeId: string;
  /** 当前 UI 节点数据。 */
  data: UiNodeData;
  /** 当前节点画布位置。 */
  position: WorkflowCanvasNode['position'];
  /** 当前节点卡片尺寸。 */
  size: WorkflowCanvasNode['size'];
  /** 当前节点可以引用的资源。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 通过统一 Flow 事务写回节点。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  /** 打开 AQL 或表达式文档。 */
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
  /** 删除当前节点。 */
  onDelete: () => void;
}>;

/** UI 节点专用的 Intent Inspector；入口只负责编排各职责区块。 */
export function ActionNodeInspector({
  nodeId,
  data,
  position,
  size,
  resourceCatalog,
  onUpdate,
  onOpenStructuredEditor,
  onDelete,
}: ActionNodeInspectorProps) {
  const viewModel = buildActionInspectorViewModel(
    data.operation,
    data.execution,
    resourceCatalog,
    data.invalid ?? false,
  );
  /** 只在节点仍为 UI 类型时写回操作，避免异步编辑覆盖类型切换。 */
  const updateOperation = (operation: UiNodeData['operation']) => onUpdate((current) => (
    current.kind === 'ui' ? { ...current, operation, invalid: false } : current
  ));
  /** 执行预算与目标语义保持独立写回。 */
  const updateExecution = (execution: UiNodeData['execution']) => onUpdate((current) => (
    current.kind === 'ui' ? { ...current, execution, invalid: false } : current
  ));

  return (
    <>
      <NodeInspectorHeader
        label={data.label}
        summary={viewModel.summary}
        runState={data.runState ?? 'idle'}
        invalid={data.invalid ?? false}
        onLabelChange={(label) => onUpdate((current) => ({ ...current, label }))}
      />
      <ActionNodeFields
        nodeId={nodeId}
        operation={data.operation}
        execution={data.execution}
        resourceCatalog={resourceCatalog}
        invalid={data.invalid}
        onChange={updateOperation}
        onExecutionChange={updateExecution}
        onOpenEditor={onOpenStructuredEditor}
      />
      <NodeOutputSection data={data} onUpdate={onUpdate} />
      <InspectorSection title="执行方式">
        <ActionAdvancedSection
          operation={data.operation}
          execution={data.execution}
          viewModel={viewModel}
          onOperationChange={updateOperation}
          onExecutionChange={updateExecution}
        />
      </InspectorSection>
      <NodeDeveloperSection
        nodeId={nodeId}
        data={data}
        position={position}
        size={size}
        nodeTypeLabel="操作界面"
      />
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除节点" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}
