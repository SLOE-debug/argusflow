import { useEffect, useState, type ChangeEvent, type ReactNode } from 'react';
import { ArrowRight } from 'lucide-react';

import type { ConditionOperator } from '../../features/workflow/contracts';
import {
  isUnary,
  type WorkflowCanvasEdge,
  type WorkflowCanvasNode,
  type WorkflowNodeData,
} from '../../features/workflow/workflowModel';

type NodeInspectorProps = {
  workflowName: string;
  variablesDraft: string;
  variablesError: string | null;
  node: WorkflowCanvasNode | null;
  edge: WorkflowCanvasEdge | null;
  selectedCount: number;
  onNameChange: (name: string) => void;
  onVariablesChange: (draft: string) => void;
  onUpdateNode: (data: Partial<WorkflowNodeData>) => void;
  onUpdateEdgeBranch: (branch: 'true' | 'false') => void;
  onDelete: () => void;
};

type WorkflowSettingsProps = Pick<
  NodeInspectorProps,
  'workflowName' | 'variablesDraft' | 'variablesError' | 'onNameChange' | 'onVariablesChange'
>;

type NodeFieldsProps = {
  node: WorkflowCanvasNode;
  onUpdate: (data: Partial<WorkflowNodeData>) => void;
  onDelete: () => void;
};

type EdgeFieldsProps = {
  edge: WorkflowCanvasEdge;
  onBranchChange: (branch: 'true' | 'false') => void;
  onDelete: () => void;
};

type FieldProps = {
  label: string;
  children: ReactNode;
};

const operatorLabels: Record<ConditionOperator, string> = {
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

/** 检查器输入控件的统一 Tailwind 样式。 */
const controlClassName =
  'w-full rounded-lg border border-slate-300 bg-white px-2.5 text-[13px] ' +
  'font-normal text-slate-800 outline-none focus:border-blue-400 ' +
  'focus:ring-[3px] focus:ring-blue-100';

/** 检查器说明信息的统一 Tailwind 样式。 */
const helpClassName =
  'm-0 rounded-lg bg-slate-100 px-2.5 py-2 text-[11px] leading-relaxed text-slate-600';

/** 节点类型对应的检查器摘要色。 */
const badgeTones: Record<WorkflowNodeData['kind'], string> = {
  start: 'border-emerald-500 bg-emerald-50 text-emerald-700',
  end: 'border-rose-500 bg-rose-50 text-rose-700',
  condition: 'border-violet-500 bg-violet-50 text-violet-700',
  delay: 'border-orange-500 bg-orange-50 text-orange-700',
  log: 'border-blue-500 bg-blue-50 text-blue-700',
};

/** 工作流、节点和边共用的右侧属性检查器。 */
export function NodeInspector(props: NodeInspectorProps) {
  const inspectorTitle = props.node
    ? '节点属性'
    : props.edge
      ? '连线属性'
      : props.selectedCount > 1
        ? '多项选择'
        : '工作流设置';

  return (
    <aside
      className={
        'z-10 flex min-w-0 flex-col overflow-hidden border-l ' +
        'border-slate-300/80 bg-slate-50 px-2.5 pt-3 pb-2'
      }
    >
      <div className="flex min-h-[34px] items-center">
        <div className="flex flex-col justify-center">
          <span className="text-[10px] leading-tight font-extrabold tracking-[.15em] text-blue-600">
            INSPECTOR
          </span>
          <h2 className="mt-0.5 text-base leading-tight font-bold">{inspectorTitle}</h2>
        </div>
      </div>
      <div className="mt-2.5 min-h-0 flex-1 overflow-auto">
        {!props.node && !props.edge && props.selectedCount <= 1 && (
          <WorkflowSettings
            workflowName={props.workflowName}
            variablesDraft={props.variablesDraft}
            variablesError={props.variablesError}
            onNameChange={props.onNameChange}
            onVariablesChange={props.onVariablesChange}
          />
        )}
        {props.selectedCount > 1 && <MultipleSelection count={props.selectedCount} />}
        {props.node && (
          <NodeFields
            node={props.node}
            onUpdate={props.onUpdateNode}
            onDelete={props.onDelete}
          />
        )}
        {props.edge && (
          <EdgeFields
            edge={props.edge}
            onBranchChange={props.onUpdateEdgeBranch}
            onDelete={props.onDelete}
          />
        )}
      </div>
    </aside>
  );
}

/** 展示多节点选择时可执行的操作提示。 */
function MultipleSelection({ count }: { count: number }) {
  return (
    <div
      className={
        'rounded-xl border border-dashed border-slate-300 px-3 py-5 ' +
        'text-center text-slate-600'
      }
    >
      <strong>{count} 个节点</strong>
      <p className="mt-1 text-xs">右键画布，通过“排列与对齐”调整节点。</p>
    </div>
  );
}

/** 编辑工作流级名称和 JSON 变量。 */
function WorkflowSettings({
  workflowName,
  variablesDraft,
  variablesError,
  onNameChange,
  onVariablesChange,
}: WorkflowSettingsProps) {
  const formatVariables = () => {
    onVariablesChange(JSON.stringify(JSON.parse(variablesDraft), null, 2));
  };

  return (
    <div className="flex flex-col gap-3">
      <Field label="工作流名称">
        <input
          className={`${controlClassName} h-9`}
          value={workflowName}
          onChange={(event) => onNameChange(event.target.value)}
        />
      </Field>
      <Field label="JSON 变量">
        <textarea
          className={`${controlClassName} resize-y py-2 font-mono leading-relaxed`}
          rows={13}
          spellCheck={false}
          value={variablesDraft}
          onChange={(event) => onVariablesChange(event.target.value)}
        />
      </Field>
      {variablesError ? (
        <p className="-mt-1 text-xs leading-relaxed text-rose-600">{variablesError}</p>
      ) : (
        <button
          type="button"
          className={
            'flex h-8 items-center justify-center self-start rounded-lg border ' +
            'border-slate-300 bg-white px-2.5 text-xs text-slate-600 hover:bg-slate-50'
          }
          onClick={formatVariables}
        >
          格式化 JSON
        </button>
      )}
      <p className={helpClassName}>条件节点使用 RFC 6901 JSON Pointer 读取这里的值。</p>
    </div>
  );
}

/** 编辑当前选中节点的通用字段及其类型专属字段。 */
function NodeFields({ node, onUpdate, onDelete }: NodeFieldsProps) {
  const [operandDraft, setOperandDraft] = useState(
    JSON.stringify(node.data.operand ?? null, null, 2),
  );
  const [operandError, setOperandError] = useState<string | null>(null);

  useEffect(() => {
    setOperandDraft(JSON.stringify(node.data.operand ?? null, null, 2));
    setOperandError(null);
  }, [node.id, node.data.operand]);

  const updateOperand = (event: ChangeEvent<HTMLTextAreaElement>) => {
    const draft = event.target.value;
    setOperandDraft(draft);

    try {
      onUpdate({ operand: JSON.parse(draft), invalid: false });
      setOperandError(null);
    } catch (error) {
      setOperandError(error instanceof Error ? error.message : 'JSON 格式无效');
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div
        className={
          'rounded-lg border-l-[3px] px-2.5 py-2 text-sm font-bold ' +
          badgeTones[node.kind]
        }
      >
        {node.data.label}
        <small className="mt-0.5 block truncate text-[10px] font-normal text-slate-500">
          {node.id}
        </small>
      </div>
      <Field label="显示名称">
        <input
          className={`${controlClassName} h-9`}
          value={node.data.label}
          onChange={(event) => onUpdate({ label: event.target.value })}
        />
      </Field>
      {node.kind === 'log' && (
        <Field label="日志内容">
          <textarea
            className={`${controlClassName} resize-y py-2 leading-relaxed`}
            rows={4}
            value={node.data.message ?? ''}
            onChange={(event) =>
              onUpdate({ message: event.target.value, invalid: false })
            }
          />
        </Field>
      )}
      {node.kind === 'delay' && (
        <Field label="等待毫秒（1–60000）">
          <input
            className={`${controlClassName} h-9`}
            type="number"
            min={1}
            max={60000}
            value={node.data.milliseconds ?? 0}
            onChange={(event) =>
              onUpdate({ milliseconds: Number(event.target.value), invalid: false })
            }
          />
        </Field>
      )}
      {node.kind === 'condition' && (
        <>
          <Field label="JSON Pointer">
            <input
              className={`${controlClassName} h-9`}
              placeholder="/user/active"
              value={node.data.pointer ?? ''}
              onChange={(event) =>
                onUpdate({ pointer: event.target.value, invalid: false })
              }
            />
          </Field>
          <Field label="运算符">
            <select
              className={`${controlClassName} h-9`}
              value={node.data.operator ?? 'equal'}
              onChange={(event) =>
                onUpdate({
                  operator: event.target.value as ConditionOperator,
                  invalid: false,
                })
              }
            >
              {Object.entries(operatorLabels).map(([value, label]) => (
                <option
                  key={value}
                  value={value}
                >
                  {label}
                </option>
              ))}
            </select>
          </Field>
          {!isUnary(node.data.operator) && (
            <Field label="右操作数（JSON）">
              <textarea
                className={`${controlClassName} resize-y py-2 font-mono leading-relaxed`}
                rows={5}
                value={operandDraft}
                onChange={updateOperand}
              />
              {operandError && (
                <span className="text-xs leading-relaxed text-rose-600">
                  {operandError}
                </span>
              )}
            </Field>
          )}
        </>
      )}
      <DeleteButton
        label="删除节点"
        onClick={onDelete}
      />
    </div>
  );
}

/** 编辑当前选中边的条件分支。 */
function EdgeFields({ edge, onBranchChange, onDelete }: EdgeFieldsProps) {
  return (
    <div className="flex flex-col gap-3">
      <div
        className={
          'flex items-center gap-2 rounded-lg bg-slate-100 p-2.5 ' +
          'text-[11px] text-slate-600'
        }
      >
        <span className="min-w-0 flex-1 truncate">{edge.source.nodeId}</span>
        <ArrowRight
          className="size-5 shrink-0 text-blue-600"
          aria-hidden="true"
        />
        <span className="min-w-0 flex-1 truncate text-right">{edge.target.nodeId}</span>
      </div>
      {edge.data.branch && (
        <Field label="条件分支">
          <select
            className={`${controlClassName} h-9`}
            value={edge.data.branch}
            onChange={(event) =>
              onBranchChange(event.target.value as 'true' | 'false')
            }
          >
            <option value="true">True</option>
            <option value="false">False</option>
          </select>
        </Field>
      )}
      <p className={helpClassName}>
        悬停连线并拖动两端锚点，可以更改 Origin 或 Target。
      </p>
      <DeleteButton
        label="删除连线"
        onClick={onDelete}
      />
    </div>
  );
}

/** 检查器中节点和连线共用的危险操作按钮。 */
function DeleteButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className={
        'flex h-9 items-center justify-center rounded-lg border border-rose-200 ' +
        'bg-rose-50 text-[13px] font-bold text-rose-700 hover:bg-rose-100'
      }
      onClick={onClick}
    >
      {label}
    </button>
  );
}

/** 为检查器控件提供统一标签布局。 */
function Field({ label, children }: FieldProps) {
  return (
    <label className="flex flex-col gap-1.5 text-xs font-semibold text-slate-600">
      <span>{label}</span>
      {children}
    </label>
  );
}
