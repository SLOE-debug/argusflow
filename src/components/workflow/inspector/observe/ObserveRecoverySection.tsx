import type { ObserveSpec } from '../../../../features/workflow';
import { Input, Switch } from '../../../ui';
import { InspectorSection } from '../InspectorControls';

type ObserveRecoverySectionProps = Readonly<{
  /** 当前观察契约。 */
  observation: ObserveSpec;
  /** 写回完整观察契约。 */
  onChange: (observation: ObserveSpec) => void;
}>;

/** 用一个开关和自然时间单位表达是否等待，不再要求选择抽象策略枚举。 */
export function ObserveRecoverySection({
  observation,
  onChange,
}: ObserveRecoverySectionProps) {
  const policy = observation.policy;
  const waitsForResult = policy.mode === 'bounded';
  return (
    <InspectorSection title="找不到时">
      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1.5 rounded-md bg-slate-50 px-2.5 py-1.5">
        <label className="flex items-center gap-2 text-[12px] font-medium text-slate-700">
          <Switch
            aria-label="等待结果出现"
            checked={waitsForResult}
            onChange={(event) => onChange({
              ...observation,
              policy: event.target.checked
                ? { mode: 'bounded', timeout_ms: 5_000, poll_interval_ms: 150 }
                : { mode: 'once' },
            })}
          />
          等待结果出现
        </label>
        {policy.mode === 'bounded' ? (
          <label className="ml-auto flex items-center gap-1.5 text-[11px] text-slate-500">
            <span>最多</span>
            <Input
              aria-label="最多等待结果秒数"
              type="number"
              min={0.1}
              max={600}
              step={0.1}
              value={policy.timeout_ms / 1_000}
              endAdornment={<span className="text-[10px] text-slate-400">秒</span>}
              containerClassName="w-[88px] border-slate-300 bg-white"
              className="tabular-nums"
              onChange={(event) => onChange({
                ...observation,
                policy: {
                  ...policy,
                  timeout_ms: Math.round(Number(event.target.value) * 1_000),
                },
              })}
            />
          </label>
        ) : null}
      </div>
      <p className="m-0 px-2.5 text-[10px] leading-4 text-slate-500">
        {waitsForResult ? '在等待时间内重复检查。' : '只检查一次，不等待界面变化。'}
      </p>
    </InspectorSection>
  );
}
