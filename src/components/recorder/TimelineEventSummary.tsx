import { diagnosticLabel, preciseDurationLabel, eventLabel, chordLabel, latestAt, type RawTraceEvent } from '../../features/recorder';
import Keyboard from 'lucide-react/dist/esm/icons/keyboard.mjs';
import MousePointer2 from 'lucide-react/dist/esm/icons/mouse-pointer-2.mjs';

/** 当前事件的输入和定位依据；只显示实际采集的属性，不生成未经验证的选择器。 */
export function TimelineEventSummary({ events, time }: Readonly<{
  events: readonly RawTraceEvent[];
  time: number;
}>) {
  const event = latestAt(events, time, (item) => item.elapsed_ms);
  const snapshot = event?.evidence?.ui_snapshot;
  const entity = snapshot?.entity;
  const semantics = entity?.semantics;
  /** 摘要优先显示稳定定位标识；完整属性保留在悬停说明中。 */
  /** 保持来源标记，UIA 属性与 DOM 属性不互相冒充。 */
  const facts = [
    ['角色', semantics?.role], ['名称', semantics?.name],
    ['AutomationId', semantics?.automation_id], ['data-testid', semantics?.test_id],
    ['DOM id', semantics?.stable_id], ['ClassName', semantics?.class_name],
    ['FrameworkId', semantics?.framework_id], ['页面', entity?.page_url],
  ].filter(([, value]) => value);
  const primaryFact = facts.find(([label]) => label === 'AutomationId' || label === 'data-testid' || label === 'DOM id') ?? facts[0];
  const diagnostics = [...event?.diagnostics ?? [], ...event?.evidence?.diagnostics ?? []].map(diagnosticLabel).join(' · ');
  const keyboard = event?.input.type === 'key' || event?.input.type === 'clipboard';
  const action = event?.input.type === 'key' && event.input.chord
    ? chordLabel(event.input.chord).replaceAll('+', ' + ')
    : event ? eventLabel(event.input) : '录制开始';
  const target = semantics?.name || event?.evidence?.context?.title;
  return (
    <div className="flex min-h-10 min-w-0 items-center gap-5 border-b border-[#dfe3ec] px-3 py-2 text-xs">
      {keyboard ? <Keyboard className="h-4 w-[18px] shrink-0 text-[#8361ff]" /> : <MousePointer2 className="h-4 w-[18px] shrink-0 text-[#a6acb7]" />}
      <div className="flex min-w-0 flex-1 items-center gap-4">
        <span className="shrink-0 tabular-nums text-[#283348]">{preciseDurationLabel(event?.elapsed_ms ?? 0)}</span>
        <p className="truncate text-[14px] font-medium text-[#283348]" title={action}>{action}</p>
        {target ? <p className="min-w-0 truncate border-l border-[#d8dde7] pl-4 text-[#70798a]" title={target}>{target}</p> : null}
      </div>
      <div className="flex min-w-0 max-w-[40%] items-center gap-3.5 text-[#697386]" title={facts.map(([label, value]) => `${label}: ${value}`).join('\n')}>
        {snapshot ? <span className="shrink-0 rounded-md bg-[#edeef4] px-3 py-1 text-[10px]">{snapshot.backend === 'uia' ? 'UIA' : 'CDP'}</span> : null}
        {primaryFact ? (
          <dl className="flex min-w-0 items-center gap-1">
            <dt className="shrink-0 after:content-[':']">{primaryFact[0]}</dt>
            <dd className="truncate">{primaryFact[1]}</dd>
          </dl>
        ) : <span className="truncate text-slate-400">未取得定位信息</span>}
      </div>
      {diagnostics ? <p className="max-w-48 truncate text-amber-700" title={diagnostics}>{diagnostics}</p> : null}
    </div>
  );
}
