import { BACKEND_LABELS, diagnosticLabel, type RawTraceEvent } from '../../features/recorder';
import { ScreenshotPreview } from './ScreenshotPreview';
import { PointerMotionDetails } from './PointerMotionDetails';
import { EventScreenFrames } from './EventScreenFrames';

/** 事件输入、窗口、结构化事实与像素证据并列展示，不推断执行目标。 */
export function RecordingEventDetails({ event, recordingId }: Readonly<{ event: RawTraceEvent; recordingId: string }>) {
  if (event.input.type === 'pointer_motion') {
    return (
      <PointerMotionDetails
        event={event}
        motion={event.input}
      />
    );
  }
  const evidence = event.evidence;
  const context = evidence?.context;
  const snapshot = evidence?.ui_snapshot;
  const entity = snapshot?.entity;
  const screenshot = evidence?.screenshot;
  /** 原窗口仍是点击目标时优先展示焦点切换后的图；换窗口则优先展示目标帧。 */
  const postStillShowsTarget = screenshot?.click_color != null;
  /** 缺失字段不以推测值填充。 */
  const facts = [
    ['时间', `${event.elapsed_ms} ms`], ['应用', context?.executable_path], ['窗口', context?.title],
    ['进程 PID', context?.window.process_id], ['窗口 HWND', context?.window.handle],
    ['结构化来源', snapshot ? BACKEND_LABELS[snapshot.backend] : '未取得可靠结构化快照'],
    ['角色', entity?.semantics.role], ['名称', entity?.semantics.name],
    ['AutomationId', entity?.semantics.automation_id], ['data-testid', entity?.semantics.test_id],
    ['DOM id', entity?.semantics.stable_id], ['ClassName', entity?.semantics.class_name],
    ['FrameworkId', entity?.semantics.framework_id], ['页面', entity?.page_url],
    ['元素范围', entity ? JSON.stringify(entity.bounds) : null],
    ['结构化采样', snapshot ? `${snapshot.observed_at_ms} ms · 耗时 ${snapshot.observation_duration_ms} ms` : null],
    ['图像采样', screenshot ? `${screenshot.captured_at_ms} ms · 耗时 ${screenshot.capture_duration_ms} ms` : '未保存截图'],
    ['屏幕范围', screenshot ? JSON.stringify(screenshot.screen_bounds) : null],
  ] as const;
  const diagnostics = [...event.diagnostics, ...evidence?.diagnostics ?? []];
  return (
    <article className="min-w-0 space-y-4 p-4">
      <h3 className="text-sm font-semibold">操作记录 {event.sequence} · 详情</h3>
      {evidence && (
        <EventScreenFrames
          key={event.sequence}
          recordingId={recordingId}
          before={evidence.screen.before}
          after={evidence.screen.after}
        />
      )}
      {event.input.type === 'window' && event.input.change === 'appeared' ? (
        <p className="text-xs text-slate-500">检测到窗口显示，不一定是新打开的窗口。操作记录按发生时间排列。</p>
      ) : null}
      <dl className="grid grid-cols-[7rem_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
        {facts.filter(([, value]) => value !== null && value !== undefined && value !== '').map(([label, value]) => (
          <div
            key={label}
            className="contents"
          >
            <dt className="text-slate-500">{label}</dt>
            <dd className="min-w-0 break-words whitespace-pre-wrap text-slate-800">{value}</dd>
          </div>
        ))}
      </dl>
      {screenshot && postStillShowsTarget ? (
        <ScreenshotPreview
          recordingId={recordingId}
          sequence={event.sequence}
          screenshot={screenshot}
        />
      ) : null}
      {evidence?.click_target ? (
        <ScreenshotPreview
          recordingId={recordingId}
          sequence={event.sequence}
          screenshot={evidence.click_target}
          target
        />
      ) : null}
      {event.input.type === 'mouse' && event.input.button === 'left' && event.input.phase === 'down'
        && !evidence?.click_target && !postStillShowsTarget ? (
          <p className="text-xs text-amber-800">未取得点击目标画面，下方仅为操作结果。</p>
        ) : null}
      {screenshot && !postStillShowsTarget ? (
        <ScreenshotPreview
          recordingId={recordingId}
          sequence={event.sequence}
          screenshot={screenshot}
        />
      ) : null}
      {diagnostics.length > 0 ? (
        <ul className="space-y-1 text-xs text-amber-800">
          {diagnostics.map((item, index) => <li key={index}>{diagnosticLabel(item)}</li>)}
        </ul>
      ) : null}
      <details className="text-xs">
        <summary className="cursor-pointer text-slate-600">查看技术详情</summary>
        <pre className="mt-2 overflow-auto rounded-md bg-slate-50 p-2">{JSON.stringify(event, null, 2)}</pre>
      </details>
    </article>
  );
}
