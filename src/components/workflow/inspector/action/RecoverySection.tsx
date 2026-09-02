import type {
  TargetLocatorKind,
  UiExecutionPolicy,
} from '../../../../features/workflow';
import { createTargetWaitPolicy } from '../../../../features/workflow';
import { Input, Switch } from '../../../ui';
import {
  InspectorSection,
} from '../InspectorControls';

type RecoverySectionProps = Readonly<{
  /** 查询目标才拥有等待出现语义。 */
  locatorKind: TargetLocatorKind;
  /** 当前节点自己的目标等待预算。 */
  execution: UiExecutionPolicy;
  /** 写回新执行预算。 */
  onChange: (execution: UiExecutionPolicy) => void;
}>;

/** 只回答“目标暂时找不到怎么办”的恢复区块。 */
export function RecoverySection({
  locatorKind,
  execution,
  onChange,
}: RecoverySectionProps) {
  if (locatorKind !== 'query') return null;
  const policy = execution.target_wait;
  const enabled = policy.mode === 'bounded';
  return (
    <InspectorSection title="找不到时">
      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1.5 rounded-md bg-slate-50 px-2.5 py-1.5">
        <label className="flex items-center gap-2 text-[12px] font-medium text-slate-700">
          <Switch
            aria-label="等待目标出现"
            checked={enabled}
            onChange={(event) => onChange({
              ...execution,
              target_wait: event.target.checked
                ? createTargetWaitPolicy(locatorKind)
                : { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
            })}
          />
          等待目标出现
        </label>
        {enabled ? (
          <label className="ml-auto flex items-center gap-1.5 text-[11px] text-slate-500">
            <span>最多</span>
            <Input
              aria-label="最多等待目标秒数"
              type="number"
              min={0.1}
              max={600}
              step={0.1}
              value={policy.timeout_ms / 1_000}
              endAdornment={<span className="text-[10px] text-slate-400">秒</span>}
              containerClassName="w-[88px] border-slate-300 bg-white"
              className="tabular-nums"
              onChange={(event) => onChange({
                ...execution,
                target_wait: {
                  ...policy,
                  timeout_ms: Math.round(Number(event.target.value) * 1_000),
                },
              })}
            />
          </label>
        ) : null}
      </div>
      {enabled ? (
        <p className="m-0 px-2.5 text-[10px] leading-4 text-slate-500">
          超时后，节点失败并停止后续步骤。
        </p>
      ) : null}
    </InspectorSection>
  );
}
