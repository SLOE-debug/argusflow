import { Plus, Trash2 } from 'lucide-react';

import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { getNativeNodeValueOutputs } from '../../../../features/workflow';
import { Input } from '../../../ui';
import { InspectorField } from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

/** 设置可供后续节点使用的额外输出。 */
export function NodeOutputBindingsFields({
  data,
  onUpdate,
}: Readonly<{
  data: WorkflowNodeData;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>) {
  const bindings = Object.entries(data.outputBindings);
  const nativeOutputNames = new Set(
    getNativeNodeValueOutputs(data).map((output) => output.name),
  );
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center">
        <p className="pr-2 text-[10px] leading-4 text-slate-500">
          把节点结果另存为输出，供后面的步骤使用。
        </p>
        <button
          type="button"
          aria-label="添加输出"
          className="ml-auto flex size-6 shrink-0 items-center justify-center rounded text-blue-600 hover:bg-blue-50"
          onClick={() => onUpdate((current) => {
            const name = createOutputName(current.outputBindings);
            return {
              ...current,
              outputBindings: {
                ...current.outputBindings,
                [name]: { type: 'expression', source: 'result' },
              },
              invalid: false,
            };
          })}
        >
          <Plus className="size-3.5 shrink-0" aria-hidden="true" />
        </button>
      </div>
      {bindings.length === 0 ? (
        <p className="rounded-md border border-dashed border-slate-200 px-2.5 py-3 text-center text-[10px] text-slate-400">
          还没有额外输出
        </p>
      ) : null}
      {bindings.map(([name, expression]) => (
        <div
          key={name}
          className="relative flex flex-col gap-2 rounded-md border border-slate-200 bg-slate-50/60 p-2.5"
        >
          <InspectorField label="输出名称">
            <Input
              aria-label={`输出 ${name} 名称`}
              value={name}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onUpdate((current) => renameOutput(
                current,
                name,
                event.target.value,
              ))}
            />
          </InspectorField>
          <ValueExprFields
            value={expression}
            literalLabel="输出值"
            literalMode="json"
            expressionLocation={{ type: 'output_binding', name }}
            onChange={(value) => onUpdate((current) => ({
              ...current,
              outputBindings: { ...current.outputBindings, [name]: value },
              invalid: false,
            }))}
          />
          {nativeOutputNames.has(name) ? (
            <p className="rounded bg-amber-50 px-2 py-1 text-[10px] leading-4 text-amber-700">
              会替换同名的默认输出。
            </p>
          ) : null}
          <button
            type="button"
            aria-label={`删除输出 ${name}`}
            className="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            onClick={() => onUpdate((current) => removeOutput(current, name))}
          >
            <Trash2 className="size-3 shrink-0" aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}

function createOutputName(bindings: Readonly<Record<string, unknown>>): string {
  let suffix = 1;
  while (Object.hasOwn(bindings, suffix === 1 ? 'output' : `output_${suffix}`)) {
    suffix += 1;
  }
  return suffix === 1 ? 'output' : `output_${suffix}`;
}

function renameOutput(
  data: WorkflowNodeData,
  previousName: string,
  nextName: string,
): WorkflowNodeData {
  const currentBindings = data.outputBindings;
  const expression = currentBindings[previousName];
  if (!expression || nextName === previousName || Object.hasOwn(currentBindings, nextName)) {
    return data;
  }
  const outputBindings = Object.fromEntries(Object.entries(currentBindings).map(([name, value]) => (
    name === previousName ? [nextName, value] : [name, value]
  )));
  return { ...data, outputBindings, invalid: false };
}

function removeOutput(data: WorkflowNodeData, name: string): WorkflowNodeData {
  const outputBindings = Object.fromEntries(
    Object.entries(data.outputBindings).filter(([candidate]) => candidate !== name),
  );
  return { ...data, outputBindings, invalid: false };
}
