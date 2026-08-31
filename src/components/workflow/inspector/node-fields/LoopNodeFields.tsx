import type { WorkflowNodeData, WorkflowNodeUpdater } from '../../../../features/workflow';
import { Input } from '../../../ui';
import { InspectorField } from '../InspectorControls';

type LoopData = Extract<WorkflowNodeData, { kind: 'loop' }>;

/** 编辑结构化循环 Gate 的次数、总时长和轮询间隔。 */
export function LoopNodeFields({
  data,
  onUpdate,
}: Readonly<{ data: LoopData; onUpdate: (updater: WorkflowNodeUpdater) => void }>) {
  const update = (fields: Partial<Pick<LoopData, 'maxIterations' | 'timeoutMs' | 'intervalMs'>>) => (
    onUpdate((current) => current.kind === 'loop'
      ? { ...current, ...fields, invalid: false }
      : current)
  );
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="最多重复次数">
        <Input
          type="number"
          min={1}
          max={10_000}
          value={data.maxIterations}
          onChange={(event) => update({ maxIterations: Number(event.target.value) })}
        />
      </InspectorField>
      <InspectorField label="最长运行时间（毫秒）">
        <Input
          type="number"
          min={1}
          max={600_000}
          value={data.timeoutMs}
          onChange={(event) => update({ timeoutMs: Number(event.target.value) })}
        />
      </InspectorField>
      <InspectorField label="每次间隔（毫秒）">
        <Input
          type="number"
          min={0}
          max={60_000}
          value={data.intervalMs}
          onChange={(event) => update({ intervalMs: Number(event.target.value) })}
        />
      </InspectorField>
      <p className="text-[10px] leading-4 text-slate-500">
        达到次数或时间上限后，流程会从“停止重复”出口继续。
      </p>
    </div>
  );
}
