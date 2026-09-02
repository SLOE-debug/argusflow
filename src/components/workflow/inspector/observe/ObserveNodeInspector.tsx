import type {
  ObservationValueType,
  ObserveSpec,
  WorkflowCanvasNode,
  WorkflowNodeData,
  WorkflowNodeUpdater,
  WorkflowResourceCatalog,
} from '../../../../features/workflow';
import {
  InspectorDeleteButton,
  InspectorSection,
} from '../InspectorControls';
import { NodeDeveloperSection } from '../NodeDeveloperSection';
import { NodeInspectorHeader } from '../NodeInspectorHeader';
import { NodeOutputSection } from '../NodeOutputSection';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';
import { ObserveAdvancedSection } from './ObserveAdvancedSection';
import { ObserveRecoverySection } from './ObserveRecoverySection';
import {
  OBSERVATION_RESULT_OPTIONS,
  ObserveTargetSection,
} from './ObserveTargetSection';

type ObserveNodeData = Extract<WorkflowNodeData, { kind: 'observe' }>;

type ObserveNodeInspectorProps = Readonly<{
  /** 当前检查节点稳定标识。 */
  nodeId: string;
  /** 当前检查节点数据。 */
  data: ObserveNodeData;
  /** 当前节点画布位置。 */
  position: WorkflowCanvasNode['position'];
  /** 当前节点卡片尺寸。 */
  size: WorkflowCanvasNode['size'];
  /** 当前节点可引用的应用和浏览器。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 通过统一 Flow 事务写回节点。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  /** 打开 AQL 文档。 */
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
  /** 删除当前节点。 */
  onDelete: () => void;
}>;

/** 检查节点使用与操作节点一致的意图面板，不再回退到通用字段堆栈。 */
export function ObserveNodeInspector({
  nodeId,
  data,
  position,
  size,
  resourceCatalog,
  onUpdate,
  onOpenStructuredEditor,
  onDelete,
}: ObserveNodeInspectorProps) {
  const summary = formatObservationSummary(data, resourceCatalog);
  /** 只在节点仍为检查类型时写回观察契约。 */
  const updateObservation = (observation: ObserveSpec) => onUpdate((current) => (
    current.kind === 'observe'
      ? { ...current, observation, invalid: false }
      : current
  ));
  /** 返回类型与 AQL 源码独立写回，Runtime 会继续校验两者一致性。 */
  const updateResultType = (resultType: ObservationValueType) => onUpdate((current) => (
    current.kind === 'observe'
      ? { ...current, resultType, invalid: false }
      : current
  ));

  return (
    <>
      <NodeInspectorHeader
        label={data.label}
        summary={summary}
        runState={data.runState ?? 'idle'}
        invalid={data.invalid ?? false}
        onLabelChange={(label) => onUpdate((current) => ({ ...current, label }))}
      />
      <ObserveTargetSection
        nodeId={nodeId}
        observation={data.observation}
        resultType={data.resultType}
        resourceCatalog={resourceCatalog}
        onObservationChange={updateObservation}
        onResultTypeChange={updateResultType}
        onOpenEditor={onOpenStructuredEditor}
      />
      <ObserveRecoverySection
        observation={data.observation}
        onChange={updateObservation}
      />
      <NodeOutputSection data={data} onUpdate={onUpdate} />
      <InspectorSection title="检查方式">
        <ObserveAdvancedSection
          observation={data.observation}
          onChange={updateObservation}
        />
      </InspectorSection>
      <NodeDeveloperSection
        nodeId={nodeId}
        data={data}
        position={position}
        size={size}
        nodeTypeLabel="检查界面"
      />
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除节点" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}

/** 摘要优先展示真实位置与返回结果，避免复述底层查询协议。 */
function formatObservationSummary(
  data: ObserveNodeData,
  catalog: WorkflowResourceCatalog,
): string {
  const resultLabel = OBSERVATION_RESULT_OPTIONS.find(
    ({ value }) => value === data.resultType,
  )?.label ?? '检查结果';
  const locationLabel = resolveLocationLabel(data.observation, catalog);
  return `在「${locationLabel}」中检查界面，返回${resultLabel}。`;
}

/** 从当前资源目录解析检查位置的用户语言名称。 */
function resolveLocationLabel(
  observation: ObserveSpec,
  catalog: WorkflowResourceCatalog,
): string {
  const scope = observation.scope;
  if (scope.type === 'current') return '当前窗口';
  const resource = catalog[scope.type].find(
    ({ nodeId }) => nodeId === scope.resource.producer_node_id,
  );
  return resource?.resourceLabel ?? scope.resource.producer_node_id;
}
