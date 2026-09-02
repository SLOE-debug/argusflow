import type {
  BackendKind,
  ObserveSpec,
} from '../../../../features/workflow';
import { Select } from '../../../ui';
import {
  InspectorField,
  InspectorMillisecondsField,
} from '../InspectorControls';

type ObservationBackendPreset = 'auto' | Extract<
  BackendKind,
  'windows_uia' | 'browser_cdp' | 'ocr_small'
>;

type ObserveAdvancedSectionProps = Readonly<{
  /** 当前观察契约。 */
  observation: ObserveSpec;
  /** 写回完整观察契约。 */
  onChange: (observation: ObserveSpec) => void;
}>;

/** 高级检查引擎选项；主路径不要求用户理解实现后端。 */
const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动（推荐）' },
  { value: 'windows_uia', label: 'Windows 控件（UIA）' },
  { value: 'browser_cdp', label: '网页元素（CDP）' },
  { value: 'ocr_small', label: '屏幕文字（OCR）' },
] as const;

/** 只承载识别引擎与检查频率，不混入开发者元数据。 */
export function ObserveAdvancedSection({
  observation,
  onChange,
}: ObserveAdvancedSectionProps) {
  const boundedPolicy = observation.policy.mode === 'bounded'
    ? observation.policy
    : null;
  return (
    <>
      <InspectorField label="检查引擎">
        <Select<ObservationBackendPreset>
          aria-label="检查引擎"
          value={resolveBackendPreset(observation)}
          options={BACKEND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(backend) => onChange({
            ...observation,
            backend_policy: backend === 'auto'
              ? { allow: [], deny: [], prefer: [] }
              : { allow: [backend], deny: [], prefer: [backend] },
          })}
        />
      </InspectorField>
      {boundedPolicy ? (
        <InspectorMillisecondsField
          label="检查间隔"
          ariaLabel="检查结果间隔"
          min={1}
          max={60_000}
          value={boundedPolicy.poll_interval_ms}
          onChange={(pollIntervalMs) => onChange({
            ...observation,
            policy: {
              ...boundedPolicy,
              poll_interval_ms: pollIntervalMs,
            },
          })}
        />
      ) : null}
    </>
  );
}

/** 只有精确单后端策略映射为用户可编辑的预设。 */
function resolveBackendPreset(observation: ObserveSpec): ObservationBackendPreset {
  const { allow, deny, prefer } = observation.backend_policy;
  const backend = allow[0];
  if (allow.length === 1 && deny.length === 0 && prefer[0] === backend
    && (backend === 'windows_uia' || backend === 'browser_cdp' || backend === 'ocr_small')) {
    return backend;
  }
  return 'auto';
}
