import type {
  AcquirePolicy,
  ActivationPolicy,
  ApplicationSpec,
  CleanupPolicy,
} from '../../features/workflow/contracts';
import { Input, Select, Textarea } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';

type ApplicationNodeFieldsProps = Readonly<{
  /** 当前应用资源获取契约。 */
  spec: ApplicationSpec;
  /** 写回字段完整的新契约。 */
  onChange: (spec: ApplicationSpec) => void;
}>;

const ACQUIRE_OPTIONS = [
  { value: 'attach_or_start', label: '连接或启动（推荐）' },
  { value: 'attach_only', label: '仅连接现有应用' },
  { value: 'always_start_new', label: '总是启动新实例' },
] as const;

const CLEANUP_OPTIONS = [
  { value: 'leave_running', label: '保持运行' },
  { value: 'close_if_started_by_workflow', label: '仅关闭本流程启动的应用' },
  { value: 'always_close', label: '总是关闭' },
] as const;

const ACTIVATION_OPTIONS = [
  { value: 'none', label: '不激活' },
  { value: 'best_effort', label: '尽力激活（推荐）' },
  { value: 'required', label: '必须激活' },
] as const;

const TITLE_MATCH_OPTIONS = [
  { value: 'equal', label: '完全相等' },
  { value: 'contains', label: '允许包含' },
] as const;

/** 编辑 Application Resource 节点的身份与生命周期策略。 */
export function ApplicationNodeFields({
  spec,
  onChange,
}: ApplicationNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="应用 EXE">
        <Input
          aria-label="应用 EXE 绝对路径"
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
      <InspectorField label="标题匹配">
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
      <InspectorField label="获取策略">
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
      <InspectorField label="启动超时">
        <Input
          aria-label="应用启动超时毫秒"
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
      <InspectorField label="清理策略">
        <Select<CleanupPolicy>
          value={spec.cleanup_policy}
          options={CLEANUP_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(cleanup_policy) => onChange({ ...spec, cleanup_policy })}
        />
      </InspectorField>
      <InspectorField label="激活策略">
        <Select<ActivationPolicy>
          value={spec.activation_policy}
          options={ACTIVATION_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(activation_policy) => onChange({ ...spec, activation_policy })}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        节点产生 session 资源；后续 UI 节点通过逻辑引用复用该会话。
      </p>
    </div>
  );
}

/** 将每行映射成一个原样 argv，避免悄悄修改参数首尾空白。 */
function parseArguments(source: string): string[] {
  return source.length === 0 ? [] : source.split('\n');
}
