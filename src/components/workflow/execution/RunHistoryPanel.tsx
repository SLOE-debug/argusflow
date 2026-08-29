import { AlertTriangle, RefreshCw } from 'lucide-react';
import { useState } from 'react';

import {
  useRunHistory,
  type ExecutionEvent,
  type RunManifest,
  type RunDetails,
  type ResolvedInputSource,
  type RunStatus,
  type RunTraceEvent,
} from '../../../features/workflow';
import { Button } from '../../ui';
import { RunTimeline } from './RunTimeline';
import { OcrArtifactViewer } from './OcrArtifactViewer';

type RunHistoryPanelProps = Readonly<{
  /** 仅用于在当前 Run 结束后触发历史列表刷新，不与历史选择状态混用。 */
  liveEvents: ReadonlyArray<ExecutionEvent>;
}>;

/** 三栏 Run Inspector：轻量 Run List、节点折叠 Timeline 与结构化 Detail。 */
export function RunHistoryPanel({ liveEvents }: RunHistoryPanelProps) {
  const history = useRunHistory(liveEvents);
  const [selectedTraceEvent, setSelectedTraceEvent] = useState<RunTraceEvent | null>(null);

  return (
    <section className="grid min-h-0 flex-1 grid-cols-[220px_minmax(240px,0.8fr)_minmax(300px,1.2fr)] divide-x divide-slate-200">
      <div className="min-h-0 overflow-y-auto px-2 py-1.5">
        <div className="mb-1.5 flex items-center justify-between">
          <h2 className="text-[10px] font-semibold text-slate-500">运行记录</h2>
          <Button
            variant="ghost"
            size="compact"
            icon={RefreshCw}
            loading={history.loading}
            loadingLabel="刷新中"
            onClick={() => void history.refresh()}
          >
            刷新
          </Button>
        </div>
        {history.error ? (
          <p className="mb-2 flex items-start gap-1 text-[11px] text-rose-700">
            <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
            {history.error}
          </p>
        ) : null}
        <div className="space-y-1">
          {history.runs.map((run) => (
            <RunListItem
              key={run.run_id}
              run={run}
              selected={run.run_id === history.selectedRunId}
              onSelect={() => {
                setSelectedTraceEvent(null);
                void history.selectRun(run.run_id);
              }}
            />
          ))}
          {!history.loading && history.runs.length === 0 ? (
            <p className="rounded-md border border-dashed border-slate-300 bg-slate-50 px-3 py-4 text-center text-[11px] text-slate-500">
              运行一次流程后，可复盘记录会显示在这里。
            </p>
          ) : null}
        </div>
      </div>
      <RunTimeline
        traceEvents={history.traceEvents}
        selectedSequence={selectedTraceEvent?.trace_sequence ?? null}
        onSelect={setSelectedTraceEvent}
      />
      <RunDetail
        run={history.selectedRun}
        traceEvent={selectedTraceEvent}
      />
    </section>
  );
}

type RunListItemProps = Readonly<{
  run: RunManifest;
  selected: boolean;
  onSelect: () => void;
}>;

function RunListItem({ run, selected, onSelect }: RunListItemProps) {
  return (
    <Button
      variant={selected ? 'secondary' : 'ghost'}
      size="compact"
      className="w-full justify-start"
      onClick={onSelect}
      title={run.run_id}
    >
      <span className={STATUS_TONES[run.status]}>{STATUS_LABELS[run.status]}</span>
      <span className="min-w-0 flex-1 truncate text-left">{run.workflow_name}</span>
      <time className="text-[10px] text-slate-400">
        {new Date(run.started_at_unix_ms).toLocaleTimeString([], {
          hour: '2-digit',
          minute: '2-digit',
        })}
      </time>
    </Button>
  );
}

type RunDetailProps = Readonly<{
  run: RunDetails | null;
  traceEvent: RunTraceEvent | null;
}>;

function RunDetail({ run, traceEvent }: RunDetailProps) {
  const manifest = run?.manifest ?? null;
  const artifacts = run?.artifacts ?? [];
  const expandedNodeId = traceEvent?.event.expanded_node_id ?? traceEvent?.event.node_id;
  const nodeTrace = expandedNodeId
    ? run?.nodes.find((node) => node.node_id === expandedNodeId) ?? null
    : null;
  const queryTrace = expandedNodeId
    ? [...(run?.query_traces ?? [])].reverse().find((trace) => trace.node_id === expandedNodeId)
      ?? null
    : run?.query_traces.slice(-1)[0] ?? null;
  return (
    <div className="min-h-0 overflow-y-auto px-3 py-1.5">
      <h3 className="mb-1.5 text-[10px] font-semibold text-slate-500">详情</h3>
      {!manifest ? (
        <p className="text-[11px] text-slate-400">请选择一次运行。</p>
      ) : traceEvent ? (
        <>
          <OcrArtifactViewer
            runId={manifest.run_id}
            artifacts={artifacts}
            queryTrace={queryTrace}
          />
          <div className="mb-2 grid grid-cols-[72px_minmax(0,1fr)] gap-x-2 gap-y-1 text-[11px]">
            <span className="text-slate-400">事件</span>
            <span className="font-medium text-slate-700">{traceEvent.event.kind}</span>
            <span className="text-slate-400">节点</span>
            <span className="text-slate-700">{traceEvent.event.node_id ?? '工作流'}</span>
            <span className="text-slate-400">时间</span>
            <span className="text-slate-700">
              {new Date(traceEvent.timestamp_unix_ms).toLocaleString()}
            </span>
          </div>
          {nodeTrace ? (
            <div className="mb-2 rounded-md border border-slate-200 bg-slate-50 p-2">
              <h4 className="mb-1 text-[10px] font-semibold text-slate-600">最终解析输入</h4>
              {nodeTrace.resolved_inputs.fields.length ? (
                <dl className="space-y-1 text-[10px]">
                  {nodeTrace.resolved_inputs.fields.map((field) => (
                    <div
                      key={field.name}
                      className="grid grid-cols-[76px_92px_minmax(0,1fr)] gap-2"
                    >
                      <dt className="font-mono text-slate-500">{field.name}</dt>
                      <dd className="truncate text-slate-500">{inputSourceLabel(field.source)}</dd>
                      <dd className="select-text break-all font-mono text-slate-700">
                        {field.redacted ? '••••••（已脱敏）' : JSON.stringify(field.value)}
                      </dd>
                    </div>
                  ))}
                </dl>
              ) : (
                <p className="text-[10px] text-slate-400">该节点没有值或资源输入。</p>
              )}
            </div>
          ) : null}
          <pre className="overflow-auto rounded-md bg-slate-950 p-2 font-mono text-[10px] leading-4 text-slate-100">
            {JSON.stringify(traceEvent, null, 2)}
          </pre>
        </>
      ) : (
        <>
        <OcrArtifactViewer
          runId={manifest.run_id}
          artifacts={artifacts}
          queryTrace={queryTrace}
        />
        <div className="grid grid-cols-[76px_minmax(0,1fr)] gap-x-2 gap-y-1 text-[11px]">
          <span className="text-slate-400">状态</span>
          <span className={STATUS_TONES[manifest.status]}>{STATUS_LABELS[manifest.status]}</span>
          <span className="text-slate-400">Run ID</span>
          <span className="select-text break-all font-mono text-slate-700">{manifest.run_id}</span>
          <span className="text-slate-400">Trace</span>
          <span className="text-slate-700">{manifest.trace_level}</span>
          <span className="text-slate-400">事件数</span>
          <span className="text-slate-700">{manifest.event_count}</span>
          {manifest.failure_message ? (
            <>
              <span className="text-slate-400">失败原因</span>
              <span className="text-rose-700">{manifest.failure_message}</span>
            </>
          ) : null}
        </div>
        </>
      )}
    </div>
  );
}

function inputSourceLabel(source: ResolvedInputSource): string {
  switch (source.type) {
    case 'literal': return '字面量';
    case 'workflow_input': return `运行输入 · ${source.key}`;
    case 'variable': return `变量 · ${source.name}`;
    case 'node': return `节点 · ${source.node_id}`;
    case 'expression': return '表达式';
    case 'resource': return `资源 · ${source.producer_node_id}.${source.output_name}`;
  }
}

const STATUS_LABELS = {
  starting: '准备',
  running: '运行中',
  completed: '成功',
  failed: '失败',
  crashed: '中断',
} satisfies Readonly<Record<RunStatus, string>>;

const STATUS_TONES = {
  starting: 'text-sky-700',
  running: 'text-blue-700',
  completed: 'text-emerald-700',
  failed: 'text-rose-700',
  crashed: 'text-amber-700',
} satisfies Readonly<Record<RunStatus, string>>;
