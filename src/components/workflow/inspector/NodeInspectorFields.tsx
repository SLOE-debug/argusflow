import ArrowRight from 'lucide-react/dist/esm/icons/arrow-right.mjs';
import { useState } from 'react';
import {
  type ControlPortId,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
} from '../../../features/workflow';
import type { FlowComponentCatalogItem } from '../../../features/workflow';
import { Select } from '../../ui';
import { ActionNodeFields } from './node-fields/ActionNodeFields';
import { ApplicationNodeFields } from './node-fields/ApplicationNodeFields';
import { BrowserNodeFields } from './node-fields/BrowserNodeFields';
import { BrowserOperationFields } from './node-fields/BrowserOperationFields';
import { CommandNodeFields } from './node-fields/CommandNodeFields';
import { ConditionNodeFields } from './node-fields/ConditionNodeFields';
import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorDeleteButton,
  InspectorField,
  InspectorSection,
} from './InspectorControls';
import { NodeOutputBindingsFields } from './node-fields/NodeOutputBindingsFields';
import { ValueExprFields } from './node-fields/ValueExprFields';
import { VariableNodeFields } from './node-fields/VariableNodeFields';
import { ComponentNodeFields } from './node-fields/ComponentNodeFields';
import { DataFormatFields } from './node-fields/DataFormatFields';
import { FailNodeFields } from './node-fields/FailNodeFields';
import { LoopNodeFields } from './node-fields/LoopNodeFields';
import { ObserveNodeFields } from './node-fields/ObserveNodeFields';
import type { StructuredEditorTarget } from '../workspace/dock/structuredEditorTarget';

type NodeInspectorFieldsProps = Readonly<{
  /** 当前唯一选中的节点。 */
  node: WorkflowCanvasNode;
  /** 可供组件实例显式升级的精确版本目录。 */
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  /** 修改节点业务字段。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  /** 请求中央工作区打开结构化文档。 */
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
  /** 删除当前节点。 */
  onDelete: () => void;
}>;

type EdgeInspectorFieldsProps = Readonly<{
  /** 当前选中的连线。 */
  edge: WorkflowCanvasEdge;
  /** 源节点决定当前端口族，不依赖端口字符串猜测语义。 */
  sourceData: WorkflowNodeData | null;
  /** 修改分支节点的控制端口。 */
  onBranchChange: (branch: ControlPortId) => void;
  /** 删除当前连线。 */
  onDelete: () => void;
}>;

/** 内置分支节点的控制端口及其用户可读名称。 */
const EDGE_BRANCH_OPTIONS = {
  boolean: [
    { value: 'true', label: '符合条件' },
    { value: 'false', label: '不符合条件' },
    { value: 'unknown', label: '暂时无法判断' },
  ],
  observation: [
    { value: 'known', label: '已获得结果' },
    { value: 'unknown', label: '暂时无法判断' },
  ],
  loop: [
    { value: 'iterate', label: '继续重复' },
    { value: 'exhausted', label: '停止重复' },
  ],
} as const;

/** 节点类型的稳定中文名称。 */
const NODE_KIND_LABELS: Readonly<Record<WorkflowNodeData['kind'], string>> = {
  start: '开始',
  log: '记录日志',
  debug: '查看结果',
  delay: '等待一段时间',
  condition: '条件判断',
  observe: '检查界面',
  loop: '重复执行',
  variable: '设置变量',
  application: '打开应用',
  browser: '打开浏览器',
  navigate: '打开网页',
  ui: '操作界面',
  command: '执行命令',
  format: '整理文本',
  component: '组合步骤',
  fail: '停止并报错',
  end: '结束',
};

/** 节点运行状态的稳定中文名称。 */
const RUN_STATE_LABELS: Readonly<Record<NonNullable<WorkflowNodeData['runState']>, string>> = {
  idle: '等待执行',
  pending: '等待运行',
  running: '正在运行',
  success: '执行成功',
  error: '执行失败',
  skipped: '未执行',
};

/** 编辑当前选中节点的基本信息和类型专属字段。 */
export function NodeInspectorFields({
  node,
  componentCatalog,
  onUpdate,
  onOpenStructuredEditor,
  onDelete,
}: NodeInspectorFieldsProps) {
  return (
    <>
      <InspectorSection title="基本信息">
        <InspectorField label="节点名称">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={node.data.label}
            onChange={(event) => {
              const label = event.target.value;
              onUpdate((current) => ({ ...current, label }));
            }}
          />
        </InspectorField>
        <InspectorField label="内部编号">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={node.id}
            readOnly
          />
        </InspectorField>
        <InspectorField label="类型">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={NODE_KIND_LABELS[node.data.kind]}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="设置">
        <NodeKindFields
          node={node}
          componentCatalog={componentCatalog}
          onUpdate={onUpdate}
          onOpenStructuredEditor={onOpenStructuredEditor}
        />
      </InspectorSection>
      <InspectorSection title="输出">
        <NodeOutputBindingsFields
          data={node.data}
          onUpdate={onUpdate}
        />
      </InspectorSection>
      <InspectorSection title="运行状态">
        <InspectorField label="状态">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={RUN_STATE_LABELS[node.data.runState ?? 'idle']}
            readOnly
          />
        </InspectorField>
        <InspectorField label="画布位置">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8 tabular-nums`}
            value={`${Math.round(node.position.x)}, ${Math.round(node.position.y)}`}
            readOnly
          />
        </InspectorField>
        <InspectorField label="卡片大小">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8 tabular-nums`}
            value={`${node.size.width} × ${node.size.height}`}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="检查结果">
        <InspectorField label="配置检查">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={node.data.invalid ? '需要修改' : '没有问题'}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="高级设置（JSON）">
        <pre className="h-[198px] overflow-auto rounded-md border border-slate-300 bg-slate-50 p-3 font-mono text-[11px] leading-5 text-slate-700">
          {JSON.stringify(node.data, null, 2)}
        </pre>
      </InspectorSection>
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除节点" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}

type NodeKindFieldsProps = Readonly<{
  node: WorkflowCanvasNode;
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
}>;

/** 根据节点判别联合穷尽渲染专属配置。 */
function NodeKindFields({
  node,
  componentCatalog,
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
      return (
        <ObserveNodeFields
          nodeId={node.id}
          observation={data.observation}
          resultType={data.resultType}
          onUpdate={onUpdate}
          onOpenEditor={onOpenStructuredEditor}
        />
      );
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
          onUpdate={onUpdate}
        />
      );
    case 'ui':
      return (
        <ActionNodeFields
          nodeId={node.id}
          operation={data.operation}
          execution={data.execution}
          onOpenEditor={onOpenStructuredEditor}
          onChange={(operation) => onUpdate((current) => current.kind === 'ui'
            ? { ...current, operation, invalid: false }
            : current)}
          onExecutionChange={(execution) => onUpdate((current) => current.kind === 'ui'
            ? { ...current, execution, invalid: false }
            : current)}
        />
      );
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
    case 'end':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>运行到这里，流程就结束了。</p>;
  }
}

/** 编辑当前选中边的分支控制端口。 */
export function EdgeInspectorFields({
  edge,
  sourceData,
  onBranchChange,
  onDelete,
}: EdgeInspectorFieldsProps) {
  const branchOptions = resolveBranchOptions(sourceData);
  return (
    <>
      <InspectorSection title="连线信息">
        <div className="flex items-center gap-2 rounded-md bg-slate-50 p-3 text-[11px] text-slate-600">
          <span className="min-w-0 flex-1 truncate">{edge.source.nodeId}</span>
          <ArrowRight className="size-4 shrink-0 text-blue-600" aria-hidden="true" />
          <span className="min-w-0 flex-1 truncate text-right">{edge.target.nodeId}</span>
        </div>
        {edge.data.branch && branchOptions ? (
          <InspectorField label="控制分支">
            <Select<ControlPortId>
              value={edge.data.branch}
              options={branchOptions}
              containerClassName="border-slate-300 bg-white"
              onValueChange={onBranchChange}
            />
          </InspectorField>
        ) : null}
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          拖动连线两端，可以更换起点或终点。
        </p>
      </InspectorSection>
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除连线" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}

/** 根据当前端口族返回可互换的分支，禁止跨节点语义改写端口。 */
function resolveBranchOptions(
  sourceData: WorkflowNodeData | null,
): ReadonlyArray<{ value: ControlPortId; label: string }> | null {
  if (sourceData?.kind === 'condition') return EDGE_BRANCH_OPTIONS.boolean.slice(0, 2);
  if (sourceData?.kind === 'loop') return EDGE_BRANCH_OPTIONS.loop;
  if (sourceData?.kind === 'observe') {
    return sourceData.resultType === 'boolean'
      ? EDGE_BRANCH_OPTIONS.boolean
      : EDGE_BRANCH_OPTIONS.observation;
  }
  return null;
}

/** 展示多节点选择时可执行的操作提示。 */
export function MultipleSelection({
  count,
  onCreateComponent,
}: Readonly<{
  count: number;
  onCreateComponent: (name: string, version: string) => boolean;
}>) {
  const [name, setName] = useState('新的组合步骤');
  const [version, setVersion] = useState('1.0.0');
  return (
    <InspectorSection title="已选择多个节点" last>
      <div className="rounded-md border border-dashed border-slate-300 px-3 py-5 text-center text-slate-600">
        <strong className="text-[13px]">已选择 {count} 个节点</strong>
        <p className="mt-1 text-[11px]">选择一段连续流程，即可保存并重复使用。</p>
      </div>
      <InspectorField label="组合步骤名称">
        <input
          className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </InspectorField>
      <InspectorField label="初始版本">
        <input
          className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8 font-mono`}
          value={version}
          onChange={(event) => setVersion(event.target.value)}
        />
      </InspectorField>
      <button
        type="button"
        className="h-8 w-full rounded bg-violet-600 text-[11px] font-semibold text-white hover:bg-violet-700"
        onClick={() => onCreateComponent(name, version)}
      >
        保存组合步骤
      </button>
    </InspectorSection>
  );
}
