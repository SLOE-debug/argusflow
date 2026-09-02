import {
  type WorkflowCanvasNode,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
  type WorkflowResourceCatalog,
} from '../../../features/workflow';
import type { FlowComponentCatalogItem } from '../../../features/workflow';
import { ApplicationNodeFields } from './node-fields/ApplicationNodeFields';
import { BrowserNodeFields } from './node-fields/BrowserNodeFields';
import { BrowserOperationFields } from './node-fields/BrowserOperationFields';
import { CommandNodeFields } from './node-fields/CommandNodeFields';
import { ConditionNodeFields } from './node-fields/ConditionNodeFields';
import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { ValueExprFields } from './node-fields/ValueExprFields';
import { VariableNodeFields } from './node-fields/VariableNodeFields';
import { ComponentNodeFields } from './node-fields/ComponentNodeFields';
import { DataFormatFields } from './node-fields/DataFormatFields';
import { FailNodeFields } from './node-fields/FailNodeFields';
import { LoopNodeFields } from './node-fields/LoopNodeFields';
import type { StructuredEditorTarget } from '../workspace/dock/structuredEditorTarget';
import { ActionNodeInspector } from './action/ActionNodeInspector';
import { GenericNodeInspector } from './GenericNodeInspector';
import {
  formatNodeInspectorSummary,
  NODE_KIND_LABELS,
  NODE_SETTINGS_TITLES,
} from './nodeInspectorPresentation';
import { ObserveNodeInspector } from './observe/ObserveNodeInspector';

type NodeInspectorFieldsProps = Readonly<{
  /** 当前唯一选中的节点。 */
  node: WorkflowCanvasNode;
  /** 可供组件实例显式升级的精确版本目录。 */
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  /** 当前节点可见的应用和浏览器资源目录。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 修改节点业务字段。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  /** 请求中央工作区打开结构化文档。 */
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
  /** 删除当前节点。 */
  onDelete: () => void;
}>;

/** 编辑当前选中节点的基本信息和类型专属字段。 */
export function NodeInspectorFields({
  node,
  componentCatalog,
  resourceCatalog,
  onUpdate,
  onOpenStructuredEditor,
  onDelete,
}: NodeInspectorFieldsProps) {
  if (node.data.kind === 'ui') {
    return (
      <ActionNodeInspector
        nodeId={node.id}
        data={node.data}
        position={node.position}
        size={node.size}
        resourceCatalog={resourceCatalog}
        onUpdate={onUpdate}
        onOpenStructuredEditor={onOpenStructuredEditor}
        onDelete={onDelete}
      />
    );
  }
  if (node.data.kind === 'observe') {
    return (
      <ObserveNodeInspector
        nodeId={node.id}
        data={node.data}
        position={node.position}
        size={node.size}
        resourceCatalog={resourceCatalog}
        onUpdate={onUpdate}
        onOpenStructuredEditor={onOpenStructuredEditor}
        onDelete={onDelete}
      />
    );
  }
  return (
    <GenericNodeInspector
      node={node}
      nodeTypeLabel={NODE_KIND_LABELS[node.data.kind]}
      settingsTitle={NODE_SETTINGS_TITLES[node.data.kind]}
      summary={formatNodeInspectorSummary(node.data)}
      onUpdate={onUpdate}
      onDelete={onDelete}
    >
      <NodeKindFields
        node={node}
        componentCatalog={componentCatalog}
        resourceCatalog={resourceCatalog}
        onUpdate={onUpdate}
        onOpenStructuredEditor={onOpenStructuredEditor}
      />
    </GenericNodeInspector>
  );
}
type NodeKindFieldsProps = Readonly<{
  node: WorkflowCanvasNode;
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  resourceCatalog: WorkflowResourceCatalog;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
}>;

/** 根据节点判别联合穷尽渲染专属配置。 */
function NodeKindFields({
  node,
  componentCatalog,
  resourceCatalog,
  onUpdate,
  onOpenStructuredEditor,
}: NodeKindFieldsProps) {
  const data = node.data;
  switch (data.kind) {
    case 'log':
      return (
        <InspectorField label="日志内容">
          <textarea
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-[76px] resize-none py-2 leading-[18px]`}
            value={data.message}
            onChange={(event) => {
              const message = event.target.value;
              onUpdate((current) => current.kind === 'log'
                ? { ...current, message, invalid: false }
                : current);
            }}
          />
        </InspectorField>
      );
    case 'debug':
      return (
        <ValueExprFields
          value={data.value}
          literalLabel="调试值"
          literalMode="json"
          expressionLocation={{ type: 'debug_value' }}
          onChange={(value) => {
            onUpdate((current) => current.kind === 'debug'
              ? { ...current, value, invalid: false }
              : current);
          }}
        />
      );
    case 'delay':
      return (
        <InspectorField label="暂停时间（毫秒）">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            type="number"
            min={1}
            max={60_000}
            value={data.milliseconds}
            onChange={(event) => {
              const milliseconds = Number(event.target.value);
              onUpdate((current) => current.kind === 'delay'
                ? { ...current, milliseconds, invalid: false }
                : current);
            }}
          />
        </InspectorField>
      );
    case 'condition':
      return (
        <ConditionNodeFields
          data={data}
          onUpdate={onUpdate}
        />
      );
    case 'observe':
      return null;
    case 'loop':
      return <LoopNodeFields data={data} onUpdate={onUpdate} />;
    case 'variable':
      return (
        <VariableNodeFields
          data={data}
          onUpdate={onUpdate}
        />
      );
    case 'application':
      return (
        <ApplicationNodeFields
          spec={data.spec}
          onChange={(spec) => onUpdate((current) => current.kind === 'application'
            ? { ...current, spec, invalid: false }
            : current)}
        />
      );
    case 'browser':
      return (
        <BrowserNodeFields
          spec={data.spec}
          onChange={(spec) => onUpdate((current) => current.kind === 'browser'
            ? { ...current, spec, invalid: false }
            : current)}
        />
      );
    case 'navigate':
      return (
        <BrowserOperationFields
          operation={data.operation}
          resourceCatalog={resourceCatalog}
          onUpdate={onUpdate}
        />
      );
    case 'ui':
      return null;
    case 'command':
      return (
        <CommandNodeFields
          nodeId={node.id}
          operation={data.operation}
          onOpenEditor={onOpenStructuredEditor}
          onChange={(operation) => onUpdate((current) => current.kind === 'command'
            ? { ...current, operation, invalid: false }
            : current)}
        />
      );
    case 'format':
      return (
        <DataFormatFields
          operation={data.operation}
          onUpdate={onUpdate}
        />
      );
    case 'component':
      return (
        <ComponentNodeFields
          data={data}
          componentCatalog={componentCatalog}
          onUpdate={onUpdate}
        />
      );
    case 'fail':
      return <FailNodeFields data={data} onUpdate={onUpdate} />;
    case 'start':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>流程从这里开始，不需要设置。</p>;
    case 'loopEntry':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>While 每一轮都从这里进入；固定边界不能删除。</p>;
    case 'loopContinue':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>到达这里会检查预算并开始下一轮。</p>;
    case 'loopComplete':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>到达这里会从 completed 端口离开 While。</p>;
    case 'end':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>运行到这里，流程就结束了。</p>;
  }
}
