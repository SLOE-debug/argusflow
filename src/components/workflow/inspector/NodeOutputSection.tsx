import Plus from 'lucide-react/dist/esm/icons/plus.mjs';
import Trash2 from 'lucide-react/dist/esm/icons/trash-2.mjs';

import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../features/workflow';
import { getNativeNodeValueOutputs } from '../../../features/workflow';
import { IconButton, Input } from '../../ui';
import { InspectorField, InspectorSection } from './InspectorControls';
import { ValueExprFields } from './node-fields/ValueExprFields';

type NodeOutputSectionProps = Readonly<{
  /** 当前节点数据。 */
  data: WorkflowNodeData;
  /** 通过统一 Flow 事务写回节点。 */
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 输出直接占据一个紧凑分段；空状态只保留摘要，不制造折叠栏或占位框。 */
export function NodeOutputSection({ data, onUpdate }: NodeOutputSectionProps) {
  const bindings = Object.entries(data.outputBindings);
  const nativeOutputNames = new Set(
    getNativeNodeValueOutputs(data).map((output) => output.name),
  );
  const outputSummary = bindings.length === 0 ? '使用默认结果' : `${bindings.length} 项`;

  return (
    <InspectorSection
      title="输出"
      action={(
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] font-normal text-slate-400">{outputSummary}</span>
          <IconButton
            label="添加输出"
            icon={Plus}
            className="text-blue-600 hover:bg-blue-50"
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
          />
        </div>
      )}
    >
      {bindings.length > 0 ? bindings.map(([name, expression]) => (
        <div
          key={name}
          className="relative flex flex-col gap-1.5 rounded-md border border-slate-200 bg-slate-50/60 p-2.5"
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
          <IconButton
            label={`删除输出 ${name}`}
            icon={Trash2}
            className="absolute top-1.5 right-1.5 text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            onClick={() => onUpdate((current) => removeOutput(current, name))}
          />
        </div>
      )) : null}
    </InspectorSection>
  );
}

/** 为新输出生成不与已有名称冲突的稳定默认名。 */
function createOutputName(bindings: Readonly<Record<string, unknown>>): string {
  let suffix = 1;
  while (Object.hasOwn(bindings, suffix === 1 ? 'output' : `output_${suffix}`)) {
    suffix += 1;
  }
  return suffix === 1 ? 'output' : `output_${suffix}`;
}

/** 重命名时保留原表达式，并拒绝覆盖已有输出。 */
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

/** 从额外输出映射中移除指定名称。 */
function removeOutput(data: WorkflowNodeData, name: string): WorkflowNodeData {
  const outputBindings = Object.fromEntries(
    Object.entries(data.outputBindings).filter(([candidate]) => candidate !== name),
  );
  return { ...data, outputBindings, invalid: false };
}
