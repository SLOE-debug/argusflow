import { Plus, Trash2 } from 'lucide-react';

import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { Input } from '../../../ui';
import { InspectorField } from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type VariableData = Extract<WorkflowNodeData, { kind: 'variable' }>;

/** 编辑一次成功或全部回滚的 Runtime Variables 赋值集合。 */
export function VariableNodeFields({
  data,
  onUpdate,
}: Readonly<{ data: VariableData; onUpdate: (updater: WorkflowNodeUpdater) => void }>) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center">
        <span className="text-[10px] font-medium text-slate-500">设置变量</span>
        <button
          type="button"
          aria-label="添加变量"
          className="ml-auto flex size-6 items-center justify-center rounded text-blue-600 hover:bg-blue-50"
          onClick={() => onUpdate((current) => current.kind === 'variable'
            ? {
                ...current,
                assignments: [
                  ...current.assignments,
                  { name: '', value: { type: 'literal', value: null } },
                ],
                invalid: false,
              }
            : current)}
        >
          <Plus className="size-3.5 shrink-0" aria-hidden="true" />
        </button>
      </div>
      {data.assignments.map((assignment, index) => (
        <div
          key={index}
          className="relative flex flex-col gap-2 rounded-md border border-slate-200 bg-slate-50/60 p-2.5"
        >
          <InspectorField label="变量名">
            <Input
              aria-label={`变量 ${index + 1} 名称`}
              value={assignment.name}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onUpdate((current) => current.kind === 'variable'
                ? {
                    ...current,
                    assignments: current.assignments.map((candidate, candidateIndex) => (
                      candidateIndex === index
                        ? { ...candidate, name: event.target.value }
                        : candidate
                    )),
                    invalid: false,
                  }
                : current)}
            />
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
          <button
            type="button"
            aria-label={`删除变量 ${index + 1}`}
            className="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            onClick={() => onUpdate((current) => current.kind === 'variable'
              ? {
                  ...current,
                  assignments: current.assignments.filter((_, candidateIndex) => (
                    candidateIndex !== index
                  )),
                  invalid: false,
                }
              : current)}
          >
            <Trash2 className="size-3 shrink-0" aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}
