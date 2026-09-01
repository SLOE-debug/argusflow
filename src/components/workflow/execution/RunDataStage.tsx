import Database from 'lucide-react/dist/esm/icons/database.mjs';

import type {
  ExecutionEvent,
  ResolvedInputSource,
  RunDetails,
  RunPresentationSnapshot,
} from '../../../features/workflow';

type RunDataStageProps = Readonly<{
  details: RunDetails | null;
  selectedEvent: ExecutionEvent | null;
  selectedNodeSequence: number | null;
  presentation: RunPresentationSnapshot;
}>;

/** 按节点执行序号关联输入、输出和原始事件，循环中的同节点不会复用旧详情。 */
export function RunDataStage({
  details,
  selectedEvent,
  selectedNodeSequence,
  presentation,
}: RunDataStageProps) {
  const nodeId = selectedEvent?.expanded_node_id ?? selectedEvent?.node_id ?? null;
  const nodeTrace = nodeId && selectedNodeSequence !== null
    ? details?.nodes.find((trace) => (
        trace.node_id === nodeId && trace.node_sequence === selectedNodeSequence
      )) ?? null
    : null;
  if (!selectedEvent || !nodeId) {
    return <EmptyData message="在时间线上选择一个节点事件，即可查看该次执行的数据。" />;
  }
  if (!nodeTrace) {
    return <EmptyData message="这次节点执行没有保存输入输出摘要，或实时数据仍在写入。" />;
  }
  return (
    <div className="h-full overflow-y-auto bg-slate-50 p-6">
      <div className="mx-auto max-w-5xl space-y-5">
        <header>
          <p className="text-[12px] font-medium text-blue-600">第 {nodeTrace.node_sequence} 号节点执行</p>
          <h2 className="mt-1 text-[18px] font-semibold text-slate-900">
            {presentation.node_labels[selectedEvent.node_id ?? ''] ?? selectedEvent.node_id}
          </h2>
        </header>
        <section className="rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
          <h3 className="text-[14px] font-semibold text-slate-800">输入与来源</h3>
          {nodeTrace.resolved_inputs.fields.length ? (
            <dl className="mt-4 divide-y divide-slate-100">
              {nodeTrace.resolved_inputs.fields.map((field) => (
                <div key={field.name} className="grid grid-cols-[160px_220px_minmax(0,1fr)] gap-4 py-3 text-[13px]">
                  <dt className="font-mono font-medium text-slate-700">{field.name}</dt>
                  <dd className="text-slate-500">{inputSourceLabel(field.source)}</dd>
                  <dd className="select-text break-words font-mono text-slate-800">
                    {field.redacted ? '••••••（已脱敏）' : JSON.stringify(field.value)}
                  </dd>
                </div>
              ))}
            </dl>
          ) : <p className="mt-3 text-[13px] text-slate-500">该节点没有值或资源输入。</p>}
        </section>
        <section className="rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
          <h3 className="text-[14px] font-semibold text-slate-800">输出摘要</h3>
          {nodeTrace.outputs ? (
            <div className="mt-3 grid gap-3 text-[13px] sm:grid-cols-2">
              <OutputList label="公开值" values={nodeTrace.outputs.output_names} />
              <OutputList label="资源" values={nodeTrace.outputs.resource_names} />
            </div>
          ) : <p className="mt-3 text-[13px] text-slate-500">该次执行尚未产生输出。</p>}
        </section>
        <section className="rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
          <h3 className="text-[14px] font-semibold text-slate-800">当前原始事件</h3>
          <pre className="mt-3 overflow-x-auto whitespace-pre-wrap break-all rounded-md bg-slate-950 p-4 text-[12px] leading-5 text-slate-100">
            {JSON.stringify(selectedEvent, null, 2)}
          </pre>
        </section>
      </div>
    </div>
  );
}

function OutputList({ label, values }: Readonly<{ label: string; values: ReadonlyArray<string> }>) {
  return (
    <div className="rounded-md bg-slate-50 p-3">
      <span className="font-medium text-slate-500">{label}</span>
      <p className="mt-1 break-words text-slate-800">{values.length ? values.join('、') : '无'}</p>
    </div>
  );
}

function EmptyData({ message }: Readonly<{ message: string }>) {
  return (
    <div className="flex h-full items-center justify-center bg-slate-50 p-8 text-center">
      <div>
        <Database className="mx-auto size-8 text-slate-300" aria-hidden="true" />
        <h2 className="mt-3 text-[14px] font-semibold text-slate-700">暂无节点数据</h2>
        <p className="mt-1 text-[13px] text-slate-500">{message}</p>
      </div>
    </div>
  );
}

function inputSourceLabel(source: ResolvedInputSource): string {
  switch (source.type) {
    case 'literal': return '工作流字面量';
    case 'workflow_input': return `运行输入 · ${source.key}`;
    case 'variable': return `变量 · ${source.name}`;
    case 'node': return `节点输出 · ${source.node_id}`;
    case 'expression': return `表达式 · ${source.expression}`;
    case 'resource': return `资源 · ${source.producer_node_id}.${source.output_name}`;
  }
}
