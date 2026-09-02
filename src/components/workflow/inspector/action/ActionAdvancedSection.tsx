import ArrowRight from 'lucide-react/dist/esm/icons/arrow-right.mjs';

import type {
  ActionInspectorViewModel,
  BackendPolicyPreset,
  UiExecutionPolicy,
  UiOperation,
} from '../../../../features/workflow';
import {
  changeBackendPolicy,
  resolveBackendPolicyPreset,
} from '../../../../features/workflow';
import { Select } from '../../../ui';
import {
  InspectorField,
  InspectorMillisecondsField,
} from '../InspectorControls';

type ActionAdvancedSectionProps = Readonly<{
  /** 当前操作和后端约束。 */
  operation: UiOperation;
  /** 当前执行预算。 */
  execution: UiExecutionPolicy;
  /** Intent ViewModel 提供的 planner 用户语言。 */
  viewModel: ActionInspectorViewModel;
  /** 写回后端策略。 */
  onOperationChange: (operation: UiOperation) => void;
  /** 写回高级等待设置。 */
  onExecutionChange: (execution: UiExecutionPolicy) => void;
}>;

/** 高级定位引擎选项；普通主路径不展示实现后端。 */
const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动（推荐）' },
  { value: 'windows_uia', label: 'Windows 控件（UIA）' },
  { value: 'browser_cdp', label: '网页元素（CDP）' },
  { value: 'ocr_small', label: '屏幕文字（OCR）' },
  { value: 'send_input', label: '模拟输入' },
] as const;

/** 执行方式直接展示定位、动作和重试细节，不再藏进折叠区。 */
export function ActionAdvancedSection({
  operation,
  execution,
  viewModel,
  onOperationChange,
  onExecutionChange,
}: ActionAdvancedSectionProps) {
  const backendPreset = resolveBackendPolicyPreset(operation.target.backend_policy);
  const queryTarget = operation.target.locator.type === 'query';
  const waitPolicy = execution.target_wait;
  return (
    <>
      {queryTarget ? (
        <InspectorField label="定位引擎">
          <Select<BackendPolicyPreset>
            aria-label="定位引擎"
            value={backendPreset}
            options={operation.type === 'click'
              ? BACKEND_OPTIONS
              : BACKEND_OPTIONS.filter(({ value }) => value !== 'ocr_small')}
            containerClassName="border-slate-300 bg-white"
            onValueChange={(preset) => onOperationChange(changeBackendPolicy(operation, preset))}
          />
        </InspectorField>
      ) : null}
      <ExecutionPlan
        locator={viewModel.locatorEngineLabel}
        action={viewModel.actionEngineLabel}
      />
      {waitPolicy.mode === 'bounded' ? (
        <InspectorMillisecondsField
          label="重试间隔"
          ariaLabel="重试目标间隔"
          min={1}
          max={60_000}
          value={waitPolicy.poll_interval_ms}
          onChange={(pollIntervalMs) => onExecutionChange({
            ...execution,
            target_wait: {
              ...waitPolicy,
              poll_interval_ms: pollIntervalMs,
            },
          })}
        />
      ) : null}
    </>
  );
}

/** 把定位和动作的先后关系压缩成一条可扫读的执行摘要。 */
function ExecutionPlan({
  locator,
  action,
}: Readonly<{ locator: string; action: string }>) {
  return (
    <div
      className="rounded-md bg-slate-50 px-2.5 py-1.5"
      aria-label={`当前执行计划：${locator}，然后${action}`}
    >
      <p className="mb-1 text-[10px] font-medium text-slate-400">当前计划</p>
      <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-slate-700">
        <span className="min-w-0 truncate rounded bg-white px-2 py-1 shadow-sm">{locator}</span>
        <ArrowRight className="size-3 shrink-0 text-slate-400" aria-hidden="true" />
        <span className="min-w-0 truncate rounded bg-white px-2 py-1 shadow-sm">{action}</span>
      </div>
    </div>
  );
}
