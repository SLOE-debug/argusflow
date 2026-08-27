import type {
  AcquirePolicy,
  ActivationPolicy,
  ApplicationSpec,
  CleanupPolicy,
} from '../../../../features/workflow';
import { Input, Select, Textarea } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';

type ApplicationNodeFieldsProps = Readonly<{
  /** 当前应用资源获取契约。 */
  spec: ApplicationSpec;
  /** 写回字段完整的新契约。 */
  onChange: (spec: ApplicationSpec) => void;
}>;

const ACQUIRE_OPTIONS = [
  { value: 'attach_or_start', label: '优先连接，找不到就打开（推荐）' },
  { value: 'attach_only', label: '只连接已打开的应用' },
  { value: 'always_start_new', label: '每次打开新应用' },
] as const;

const CLEANUP_OPTIONS = [
  { value: 'leave_running', label: '保持运行' },
  { value: 'close_if_started_by_workflow', label: '只关闭本次启动的应用' },
  { value: 'always_close', label: '流程结束时关闭' },
] as const;

const ACTIVATION_OPTIONS = [
  { value: 'none', label: '不切换窗口' },
  { value: 'best_effort', label: '尝试切换到前台（推荐）' },
  { value: 'required', label: '必须切换到前台' },
] as const;

const TITLE_MATCH_OPTIONS = [
  { value: 'equal', label: '完全匹配' },
  { value: 'contains', label: '包含标题' },
] as const;

/** 编辑 Application Resource 节点的身份与生命周期策略。 */
export function ApplicationNodeFields({
  spec,
  onChange,
}: ApplicationNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="应用程序">
        <Input
          aria-label="应用程序路径"
          value={spec.executable_path}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            executable_path: event.target.value,
          })}
        />
      </InspectorField>
      <InspectorField label="窗口标题">
        <Input
          aria-label="应用窗口标题"
          value={spec.window_title.value}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            window_title: { ...spec.window_title, value: event.target.value },
          })}
        />
      </InspectorField>
      <InspectorField label="匹配方式">
        <Select<'equal' | 'contains'>
          value={spec.window_title.type}
          options={TITLE_MATCH_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => onChange({
            ...spec,
            window_title: { type, value: spec.window_title.value },
          })}
        />
      </InspectorField>
      <InspectorField label="打开方式">
        <Select<AcquirePolicy>
          value={spec.acquire_policy}
          options={ACQUIRE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(acquire_policy) => onChange({ ...spec, acquire_policy })}
        />
      </InspectorField>
      <InspectorField label="启动参数">
        <Textarea
          aria-label="应用启动参数"
          className="h-[58px] resize-y border-slate-300 bg-white leading-[18px]"
          value={spec.arguments.join('\n')}
          onChange={(event) => onChange({
            ...spec,
            arguments: parseArguments(event.target.value),
          })}
        />
      </InspectorField>
      <InspectorField label="启动超时（毫秒）">
        <Input
          aria-label="应用启动超时"
          type="number"
          min={100}
          max={60_000}
          value={spec.launch_timeout_ms}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...spec,
            launch_timeout_ms: Number(event.target.value),
          })}
        />
      </InspectorField>
      <InspectorField label="结束时处理">
        <Select<CleanupPolicy>
          value={spec.cleanup_policy}
          options={CLEANUP_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(cleanup_policy) => onChange({ ...spec, cleanup_policy })}
        />
      </InspectorField>
      <InspectorField label="窗口激活">
        <Select<ActivationPolicy>
          value={spec.activation_policy}
          options={ACTIVATION_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(activation_policy) => onChange({ ...spec, activation_policy })}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        后面的界面操作会使用这个应用。
      </p>
    </div>
  );
}

/** 将每行映射成一个原样 argv，避免悄悄修改参数首尾空白。 */
function parseArguments(source: string): string[] {
  return source.length === 0 ? [] : source.split('\n');
}
