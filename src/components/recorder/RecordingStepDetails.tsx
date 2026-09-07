import { BACKEND_LABELS, BASIS_LABELS, diagnosticLabel, selectorLabel,
  type SemanticRecord } from '../../features/recorder';

/** 展示观察事实、候选评分和降级原因，不重新生成选择器。 */
export function RecordingStepDetails({ record }: Readonly<{ record: SemanticRecord }>) {
  const { target } = record;
  const context = target.context;
  const entity = target.entity;
  const facts = [
    ['应用', context?.executable_path], ['窗口', context?.title],
    ['进程 PID', context?.window.process_id], ['窗口 HWND', context?.window.handle],
    ['后端', BACKEND_LABELS[target.backend]], ['置信度', `${Math.round(target.confidence * 100)}%`],
    ['角色', entity?.semantics.role], ['名称', entity?.semantics.name],
    ['AutomationId', entity?.semantics.automation_id], ['data-testid', entity?.semantics.test_id],
    ['DOM id', entity?.semantics.stable_id], ['ClassName', entity?.semantics.class_name],
    ['FrameworkId', entity?.semantics.framework_id], ['页面', entity?.page_url],
    ['浏览器会话', entity?.browser_session],
    ['敏感性', entity?.sensitivity === 'sensitive' ? '敏感字段' : entity?.sensitivity === 'normal' ? '普通字段' : '未知'],
    ['元素 bounds', entity ? JSON.stringify(entity.bounds) : null],
    ['原始事件', record.raw_event_ids.join(', ')],
  ] as const;
  return (
    <article className="min-w-0 space-y-4 p-4">
      <h3 className="text-sm font-semibold">步骤 {record.sequence} · 定位详情</h3>
      <dl className="grid grid-cols-[7rem_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
        {facts.filter(([, value]) => value !== null && value !== undefined && value !== '').map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-slate-500">{label}</dt>
            <dd className="min-w-0 break-words whitespace-pre-wrap text-slate-800">{value}</dd>
          </div>
        ))}
      </dl>
      <section>
        <h4 className="mb-2 text-xs font-semibold">定位候选</h4>
        {target.selector_candidates.length === 0 ? <p className="text-xs text-slate-500">没有可靠定位候选。</p> : (
          <ol className="space-y-2">
            {target.selector_candidates.map((candidate, index) => (
              <li
                key={`${index}-${candidate.basis}`}
                className="rounded-md border border-slate-200 bg-slate-50 p-2"
              >
                <div className="mb-1 flex flex-wrap items-center gap-2 text-xs">
                  {target.preferred_selector === index ? <span className="font-semibold text-blue-700">首选</span> : null}
                  <span>{BASIS_LABELS[candidate.basis]}</span>
                  <span className="ml-auto text-slate-500">稳定性 {candidate.stability_score}/100</span>
                </div>
                <code className="break-all text-xs text-slate-700">{selectorLabel(candidate.selector)}</code>
              </li>
            ))}
          </ol>
        )}
      </section>
      {target.diagnostics.length > 0 ? (
        <section>
          <h4 className="mb-2 text-xs font-semibold">定位诊断</h4>
          <ul className="space-y-1 text-xs text-amber-800">
            {target.diagnostics.map((item, index) => <li key={index}>{diagnosticLabel(item)}</li>)}
          </ul>
        </section>
      ) : null}
      <details className="text-xs">
        <summary className="cursor-pointer text-slate-600">对应原始输入（已脱敏）</summary>
        <pre className="mt-2 overflow-auto rounded-md bg-slate-50 p-2">{JSON.stringify(record.original_input, null, 2)}</pre>
      </details>
    </article>
  );
}
