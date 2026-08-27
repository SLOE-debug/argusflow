import { ArrowRight } from 'lucide-react';
import {
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
} from '../../features/workflow/workflowModel';
import { ActionNodeFields } from './ActionNodeFields';
import { ApplicationNodeFields } from './ApplicationNodeFields';
import { BrowserNodeFields } from './BrowserNodeFields';
import { CommandNodeFields } from './CommandNodeFields';
import { ConditionNodeFields } from './ConditionNodeFields';
import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorDeleteButton,
  InspectorField,
  InspectorSection,
} from './InspectorControls';
import { NodeOutputBindingsFields } from './NodeOutputBindingsFields';
import { ValueExprFields } from './ValueExprFields';
import { VariableNodeFields } from './VariableNodeFields';
import type { StructuredEditorTarget } from './structuredEditorTarget';

type NodeInspectorFieldsProps = Readonly<{
  /** 当前唯一选中的节点。 */
  node: WorkflowCanvasNode;
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
  /** 修改条件分支。 */
  onBranchChange: (branch: 'true' | 'false') => void;
  /** 删除当前连线。 */
  onDelete: () => void;
}>;

/** 节点类型的稳定中文名称。 */
const NODE_KIND_LABELS: Readonly<Record<WorkflowNodeData['kind'], string>> = {
  start: '开始节点',
  log: '日志节点',
  debug: '调试输出',
  delay: '延迟节点',
  condition: '条件判断',
  variable: '设置变量',
  application: '应用资源',
  browser: '浏览器资源',
  ui: '界面操作',
  command: '命令节点',
  end: '结束节点',
};

/** 节点运行状态的稳定中文名称。 */
const RUN_STATE_LABELS: Readonly<Record<NonNullable<WorkflowNodeData['runState']>, string>> = {
  idle: '等待执行',
  pending: '排队等待',
  running: '正在运行',
  success: '执行成功',
  error: '执行失败',
  skipped: '未执行',
};

/** 编辑当前选中节点的基本信息和类型专属字段。 */
export function NodeInspectorFields({
  node,
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
        <InspectorField label="节点 ID">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={node.id}
            readOnly
          />
        </InspectorField>
        <InspectorField label="节点类型">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={NODE_KIND_LABELS[node.data.kind]}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="参数配置">
        <NodeKindFields
          node={node}
          onUpdate={onUpdate}
          onOpenStructuredEditor={onOpenStructuredEditor}
        />
      </InspectorSection>
      <InspectorSection title="公开输出">
        <NodeOutputBindingsFields
          data={node.data}
          onUpdate={onUpdate}
        />
      </InspectorSection>
      <InspectorSection title="执行状态">
        <InspectorField label="当前状态">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={RUN_STATE_LABELS[node.data.runState ?? 'idle']}
            readOnly
          />
        </InspectorField>
        <InspectorField label="节点位置">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8 tabular-nums`}
            value={`${Math.round(node.position.x)}, ${Math.round(node.position.y)}`}
            readOnly
          />
        </InspectorField>
        <InspectorField label="节点尺寸">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8 tabular-nums`}
            value={`${node.size.width} × ${node.size.height}`}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="其他设置">
        <InspectorField label="配置状态">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={node.data.invalid ? '需要修正' : '配置有效'}
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="高级配置（JSON）">
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
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  onOpenStructuredEditor: (target: StructuredEditorTarget) => void;
}>;

/** 根据节点判别联合穷尽渲染专属配置。 */
function NodeKindFields({
  node,
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
        <InspectorField label="等待毫秒">
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
    case 'ui':
      return (
        <ActionNodeFields
          nodeId={node.id}
          operation={data.operation}
          onOpenEditor={onOpenStructuredEditor}
          onChange={(operation) => onUpdate((current) => current.kind === 'ui'
            ? { ...current, operation, invalid: false }
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
    case 'start':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>开始节点是工作流的唯一入口，无额外参数。</p>;
    case 'end':
      return <p className={INSPECTOR_HELP_CLASS_NAME}>结束节点标记当前工作流正常完成。</p>;
  }
}

/** 编辑当前选中边的条件分支。 */
export function EdgeInspectorFields({
  edge,
  onBranchChange,
  onDelete,
}: EdgeInspectorFieldsProps) {
  return (
    <>
      <InspectorSection title="连线信息">
        <div className="flex items-center gap-2 rounded-md bg-slate-50 p-3 text-[11px] text-slate-600">
          <span className="min-w-0 flex-1 truncate">{edge.source.nodeId}</span>
          <ArrowRight className="size-4 shrink-0 text-blue-600" aria-hidden="true" />
          <span className="min-w-0 flex-1 truncate text-right">{edge.target.nodeId}</span>
        </div>
        {edge.data.branch ? (
          <InspectorField label="条件分支">
            <select
              className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
              value={edge.data.branch}
              onChange={(event) => onBranchChange(event.target.value as 'true' | 'false')}
            >
              <option value="true">满足条件</option>
              <option value="false">不满足条件</option>
            </select>
          </InspectorField>
        ) : null}
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          悬停连线并拖动两端锚点，可以更改起点或终点。
        </p>
      </InspectorSection>
      <InspectorSection title="危险操作" last>
        <InspectorDeleteButton label="删除连线" onClick={onDelete} />
      </InspectorSection>
    </>
  );
}

/** 展示多节点选择时可执行的操作提示。 */
export function MultipleSelection({ count }: Readonly<{ count: number }>) {
  return (
    <InspectorSection title="多项选择" last>
      <div className="rounded-md border border-dashed border-slate-300 px-3 py-5 text-center text-slate-600">
        <strong className="text-[13px]">{count} 个节点</strong>
        <p className="mt-1 text-[11px]">右键画布，通过“排列与对齐”调整节点。</p>
      </div>
    </InspectorSection>
  );
}
