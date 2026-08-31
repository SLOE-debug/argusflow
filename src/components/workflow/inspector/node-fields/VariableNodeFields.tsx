import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import Trash2 from 'lucide-react/dist/esm/icons/trash-2.mjs';

import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { IconButton, Select } from '../../../ui';
import { InspectorField } from '../InspectorControls';
import { useValueExprEditorContext, ValueExprFields } from './ValueExprFields';

type VariableData = Extract<WorkflowNodeData, { kind: 'variable' }>;

/** 编辑一次成功或全部回滚的 Runtime Variables 赋值集合。 */
export function VariableNodeFields({
  data,
  onUpdate,
}: Readonly<{ data: VariableData; onUpdate: (updater: WorkflowNodeUpdater) => void }>) {
  const editorContext = useValueExprEditorContext();
  const declaredVariableOptions = editorContext?.symbols?.variables.map((variable) => ({
    value: variable.name,
    label: variable.label,
  })) ?? [];
  const assignedVariableNames = new Set(data.assignments.map((assignment) => assignment.name));
  const nextVariableName = declaredVariableOptions.find((option) => (
    !assignedVariableNames.has(option.value)
  ))?.value;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center">
        <span className="text-[10px] font-medium text-slate-500">设置变量</span>
        <IconButton
          label={nextVariableName === undefined ? '没有可添加的已声明变量' : '添加变量赋值'}
          icon={Plus}
          className="ml-auto text-blue-600 hover:bg-blue-50"
          disabled={nextVariableName === undefined}
          onClick={() => onUpdate((current) => current.kind === 'variable'
            ? {
                ...current,
                assignments: [
                  ...current.assignments,
                  {
                    name: nextVariableName ?? '',
                    value: { type: 'literal', value: null },
                  },
                ],
                invalid: false,
              }
            : current)}
        />
      </div>
      {data.assignments.map((assignment, index) => {
        const undeclared = assignment.name.length > 0
          && !declaredVariableOptions.some((option) => option.value === assignment.name);
        const variableOptions = declaredVariableOptions.map((option) => ({
          ...option,
          disabled: option.value !== assignment.name && assignedVariableNames.has(option.value),
        }));
        return (
          <div
            key={index}
            className="relative flex flex-col gap-2 rounded-md border border-slate-200 bg-slate-50/60 p-2.5"
          >
            <InspectorField label="变量名">
              <Select<string>
                aria-label={`变量 ${index + 1} 名称`}
                value={assignment.name}
                options={variableOptions.length > 0
                  ? variableOptions
                  : [{ value: '', label: '请先在工作流数据中声明变量', disabled: true }]}
                disabled={declaredVariableOptions.length === 0}
                onValueChange={(name) => onUpdate((current) => current.kind === 'variable'
                  ? {
                      ...current,
                      assignments: current.assignments.map((candidate, candidateIndex) => (
                        candidateIndex === index
                          ? { ...candidate, name }
                          : candidate
                      )),
                      invalid: false,
                    }
                  : current)}
              />
              {undeclared ? (
                <p className="mt-1 text-[10px] text-rose-600">
                  变量“{assignment.name}”未声明，请重新选择。
                </p>
              ) : null}
            </InspectorField>
            <ValueExprFields
              value={assignment.value}
              literalLabel="变量值"
              literalMode="json"
              expressionLocation={{ type: 'variable_assignment', index }}
              onChange={(value) => onUpdate((current) => current.kind === 'variable'
                ? {
                    ...current,
                    assignments: current.assignments.map((candidate, candidateIndex) => (
                      candidateIndex === index ? { ...candidate, value } : candidate
                    )),
                    invalid: false,
                  }
                : current)}
            />
            <IconButton
              label={`删除变量赋值 ${index + 1}`}
              icon={Trash2}
              className="absolute top-1.5 right-1.5 text-slate-400 hover:bg-rose-50 hover:text-rose-600"
              onClick={() => onUpdate((current) => current.kind === 'variable'
                ? {
                    ...current,
                    assignments: current.assignments.filter((_, candidateIndex) => (
                      candidateIndex !== index
                    )),
                    invalid: false,
                  }
                : current)}
            />
          </div>
        );
      })}
    </div>
  );
}
