import { ArrowRight } from 'lucide-react';
import { useEffect, useState, type ChangeEvent } from 'react';

import type {
  ConditionOperator,
  JsonValue,
} from '../../features/workflow/contracts';
import {
  isUnary,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
} from '../../features/workflow/workflowModel';
import { ActionNodeFields } from './ActionNodeFields';
import { ApplicationNodeFields } from './ApplicationNodeFields';
import { CommandNodeFields } from './CommandNodeFields';
import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorDeleteButton,
  InspectorField,
  InspectorSection,
} from './InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type NodeInspectorFieldsProps = Readonly<{
  /** 当前唯一选中的节点。 */
  node: WorkflowCanvasNode;
  /** 修改节点业务字段。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
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

/** 条件运算符的稳定显示名称。 */
const OPERATOR_LABELS: Readonly<Record<ConditionOperator, string>> = {
  equal: '等于',
  not_equal: '不等于',
  greater_than: '大于',
  greater_than_or_equal: '大于等于',
  less_than: '小于',
  less_than_or_equal: '小于等于',
  contains: '包含',
  exists: '存在',
  not_exists: '不存在',
  is_empty: '为空',
  not_empty: '不为空',
};

/** 节点类型的稳定中文名称。 */
const NODE_KIND_LABELS: Readonly<Record<WorkflowNodeData['kind'], string>> = {
  start: '开始节点',
  log: '日志节点',
  debug: '调试输出',
  delay: '延迟节点',
  condition: '条件判断',
  application: '应用资源',
  ui: '界面操作',
  command: '命令节点',
  end: '结束节点',
};

/** 节点运行状态的稳定中文名称。 */
const RUN_STATE_LABELS: Readonly<Record<NonNullable<WorkflowNodeData['runState']>, string>> = {
  idle: '等待执行',
  running: '正在运行',
  success: '执行成功',
  error: '执行失败',
};

/** 编辑当前选中节点的基本信息和类型专属字段。 */
export function NodeInspectorFields({ node, onUpdate, onDelete }: NodeInspectorFieldsProps) {
  const conditionData = node.data.kind === 'condition' ? node.data : null;
  const [operandDraft, setOperandDraft] = useState(
    JSON.stringify(conditionData?.operand ?? null, null, 2),
  );
  const [operandError, setOperandError] = useState<string | null>(null);

  useEffect(() => {
    setOperandDraft(JSON.stringify(conditionData?.operand ?? null, null, 2));
    setOperandError(null);
  }, [conditionData?.operand, node.id]);

  const updateOperand = (event: ChangeEvent<HTMLTextAreaElement>) => {
    const draft = event.target.value;
    setOperandDraft(draft);
    try {
      /** JSON.parse 成功后结果属于契约允许的递归 JSON 值。 */
      const operand = JSON.parse(draft) as JsonValue;
      onUpdate((current) => current.kind === 'condition'
        ? { ...current, operand, invalid: false }
        : current);
      setOperandError(null);
    } catch (error) {
      setOperandError(error instanceof Error ? error.message : 'JSON 格式无效');
    }
  };

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
          operandDraft={operandDraft}
          operandError={operandError}
          onOperandChange={updateOperand}
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
  operandDraft: string;
  operandError: string | null;
  onOperandChange: (event: ChangeEvent<HTMLTextAreaElement>) => void;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 根据节点判别联合穷尽渲染专属配置。 */
function NodeKindFields({
  node,
  operandDraft,
  operandError,
  onOperandChange,
  onUpdate,
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
          literalLabel="调试文本"
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
        <>
          <InspectorField label="JSON Pointer">
            <input
              className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
              placeholder="/user/active"
              value={data.pointer}
              onChange={(event) => {
                const pointer = event.target.value;
                onUpdate((current) => current.kind === 'condition'
                  ? { ...current, pointer, invalid: false }
                  : current);
              }}
            />
          </InspectorField>
          <InspectorField label="运算符">
            <select
              className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
              value={data.operator}
              onChange={(event) => {
                const operator = event.target.value as ConditionOperator;
                onUpdate((current) => current.kind === 'condition'
                  ? { ...current, operator, invalid: false }
                  : current);
              }}
            >
              {Object.entries(OPERATOR_LABELS).map(([value, label]) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
          </InspectorField>
          {!isUnary(data.operator) ? (
            <InspectorField label="右操作数">
              <textarea
                className={`${INSPECTOR_CONTROL_CLASS_NAME} h-[76px] resize-none py-2 font-mono leading-[18px]`}
                value={operandDraft}
                onChange={onOperandChange}
              />
              {operandError ? (
                <span className="mt-1 block text-[11px] text-rose-600">{operandError}</span>
              ) : null}
            </InspectorField>
          ) : null}
        </>
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
    case 'ui':
      return (
        <ActionNodeFields
          operation={data.operation}
          onChange={(operation) => onUpdate((current) => current.kind === 'ui'
            ? { ...current, operation, invalid: false }
            : current)}
        />
      );
    case 'command':
      return (
        <CommandNodeFields
          operation={data.operation}
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
