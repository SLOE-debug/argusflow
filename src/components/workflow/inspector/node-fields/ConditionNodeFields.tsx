import type { ConditionOperator } from '../../../../features/workflow';
import {
  isUnary,
  type WorkflowNodeData,
  type WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { Select } from '../../../ui';
import { InspectorField } from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type ConditionData = Extract<WorkflowNodeData, { kind: 'condition' }>;

/** 条件运算符的稳定显示名称。 */
const OPERATOR_OPTIONS = [
  { value: 'equal', label: '等于' },
  { value: 'not_equal', label: '不等于' },
  { value: 'greater_than', label: '大于' },
  { value: 'greater_than_or_equal', label: '大于等于' },
  { value: 'less_than', label: '小于' },
  { value: 'less_than_or_equal', label: '小于等于' },
  { value: 'contains', label: '包含' },
  { value: 'exists', label: '存在' },
  { value: 'not_exists', label: '不存在' },
  { value: 'is_empty', label: '为空' },
  { value: 'not_empty', label: '不为空' },
] as const;

/** 编辑从 RunContext 求值的左右表达式与安全比较运算符。 */
export function ConditionNodeFields({
  data,
  onUpdate,
}: Readonly<{ data: ConditionData; onUpdate: (updater: WorkflowNodeUpdater) => void }>) {
  return (
    <div className="flex flex-col gap-2.5">
      <div>
        <span className="mb-1 block text-[10px] font-medium text-slate-500">左值</span>
        <ValueExprFields
          value={data.left}
          literalLabel="左值"
          literalMode="json"
          expressionLocation={{ type: 'condition_left' }}
          onChange={(left) => onUpdate((current) => current.kind === 'condition'
            ? { ...current, left, invalid: false }
            : current)}
        />
      </div>
      <InspectorField label="运算符">
        <Select<ConditionOperator>
          value={data.operator}
          options={OPERATOR_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(operator) => onUpdate((current) => {
            if (current.kind !== 'condition') return current;
            return {
              ...current,
              operator,
              right: isUnary(operator)
                ? null
                : current.right ?? { type: 'literal', value: null },
              invalid: false,
            };
          })}
        />
      </InspectorField>
      {!isUnary(data.operator) && data.right ? (
        <div>
          <span className="mb-1 block text-[10px] font-medium text-slate-500">右值</span>
          <ValueExprFields
            value={data.right}
            literalLabel="右值"
            literalMode="json"
            expressionLocation={{ type: 'condition_right' }}
            onChange={(right) => onUpdate((current) => current.kind === 'condition'
              ? { ...current, right, invalid: false }
              : current)}
          />
        </div>
      ) : null}
    </div>
  );
}
