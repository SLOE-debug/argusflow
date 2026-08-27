import { Plus, Trash2 } from 'lucide-react';

import type {
  CommandOperation,
  CommandRunner,
  EnvironmentBinding,
  ValueExpr,
} from '../../features/workflow/contracts';
import { changeCommandRunner } from '../../features/workflow/workflowCommand';
import type { ValueExprLocation } from '../../features/workflow/workflowValueExpressions';
import { Checkbox, Input, Select } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { CommandScriptField } from './CommandScriptField';
import { ValueExprFields } from './ValueExprFields';
import type { StructuredEditorTarget } from './structuredEditorTarget';

type CommandNodeFieldsProps = Readonly<{
  /** 当前命令节点的稳定标识，用于隔离 Monaco 文档。 */
  nodeId: string;
  /** 当前命令执行契约。 */
  operation: CommandOperation;
  /** 写回字段完整的新契约。 */
  onChange: (operation: CommandOperation) => void;
  /** 请求 Workspace 打开一个结构化文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

const RUNNER_OPTIONS = [
  { value: 'direct', label: '直接程序（推荐）' },
  { value: 'power_shell', label: 'PowerShell' },
  { value: 'cmd', label: 'CMD' },
] as const;

/** 编辑 Direct、PowerShell 和 CMD 共用的命令输入输出契约。 */
export function CommandNodeFields({
  nodeId,
  operation,
  onChange,
  onOpenEditor,
}: CommandNodeFieldsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="运行方式">
        <Select<CommandRunner>
          value={operation.runner}
          options={RUNNER_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(runner) => onChange(changeCommandRunner(operation, runner))}
        />
      </InspectorField>
      {operation.runner === 'direct' && operation.program ? (
        <ExpressionSection
          title="程序"
          value={operation.program}
          expressionLocation={{ type: 'command_field', field: 'program' }}
          onChange={(program) => onChange({ ...operation, program })}
        />
      ) : null}
      {operation.runner === 'direct' ? (
        <ValueList
          title="参数"
          values={operation.arguments}
          locationType="command_argument"
          onChange={(argumentsValue) => onChange({
            ...operation,
            arguments: argumentsValue,
          })}
        />
      ) : null}
      {operation.runner !== 'direct' && operation.script ? (
        <CommandScriptField
          runner={operation.runner}
          value={operation.script}
          onChange={(script) => onChange({ ...operation, script })}
          onOpenEditor={() => onOpenEditor({
            type: 'command_script',
            nodeId,
          })}
          expressionLocation={{ type: 'command_field', field: 'script' }}
        />
      ) : null}
      <OptionalExpression
        title="工作目录"
        value={operation.working_directory}
        expressionLocation={{ type: 'command_field', field: 'working_directory' }}
        onChange={(working_directory) => onChange({ ...operation, working_directory })}
      />
      <OptionalExpression
        title="标准输入"
        value={operation.stdin}
        expressionLocation={{ type: 'command_field', field: 'stdin' }}
        onChange={(stdin) => onChange({ ...operation, stdin })}
      />
      <EnvironmentFields
        bindings={operation.environment}
        onChange={(environment) => onChange({ ...operation, environment })}
      />
      <InspectorField label="超时毫秒">
        <Input
          aria-label="命令超时毫秒"
          type="number"
          min={1}
          max={3_600_000}
          value={operation.timeout_ms}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...operation,
            timeout_ms: Number(event.target.value),
          })}
        />
      </InspectorField>
      <InspectorField label="成功退出代码">
        <Input
          aria-label="命令成功退出代码"
          value={operation.accepted_exit_codes.join(', ')}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...operation,
            accepted_exit_codes: parseExitCodes(event.target.value),
          })}
        />
      </InspectorField>
      <details className="rounded-md border border-slate-200 bg-slate-50/70 px-2.5 py-2">
        <summary className="cursor-pointer select-none text-[10px] font-medium text-slate-600">
          输出资源上限
        </summary>
        <div className="mt-2 flex flex-col gap-2.5">
          <ByteLimitField
            label="stdout 最大字节"
            value={operation.max_stdout_bytes}
            onChange={(max_stdout_bytes) => onChange({
              ...operation,
              max_stdout_bytes,
            })}
          />
          <ByteLimitField
            label="stderr 最大字节"
            value={operation.max_stderr_bytes}
            onChange={(max_stderr_bytes) => onChange({
              ...operation,
              max_stderr_bytes,
            })}
          />
        </div>
      </details>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        输出端口固定为 exit_code、stdout 和 stderr；完整输出不会自动写入运行日志。
      </p>
    </div>
  );
}

/** 给一个必需值参数添加具名分组。 */
function ExpressionSection({
  title,
  value,
  expressionLocation,
  onChange,
}: Readonly<{
  title: string;
  value: ValueExpr;
  expressionLocation: ValueExprLocation;
  onChange: (value: ValueExpr) => void;
}>) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[10px] font-medium text-slate-500">{title}</span>
      <ValueExprFields
        value={value}
        literalLabel={title}
        expressionLocation={expressionLocation}
        onChange={onChange}
      />
    </div>
  );
}

/** 编辑可选值表达式，并通过开关决定是否序列化字段。 */
function OptionalExpression({
  title,
  value,
  expressionLocation,
  onChange,
}: Readonly<{
  title: string;
  value: ValueExpr | null;
  expressionLocation: ValueExprLocation;
  onChange: (value: ValueExpr | null) => void;
}>) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="flex h-7 items-center gap-2 text-[10px] font-medium text-slate-600">
        <Checkbox
          checked={value !== null}
          onChange={(event) => onChange(event.target.checked
            ? { type: 'literal', value: '' }
            : null)}
        />
        {title}
      </label>
      {value ? (
        <ValueExprFields
          value={value}
          literalLabel={title}
          expressionLocation={expressionLocation}
          onChange={onChange}
        />
      ) : null}
    </div>
  );
}

/** 编辑可变数量的 Direct 参数，每项保持独立 ValueExpr。 */
function ValueList({
  title,
  values,
  locationType,
  onChange,
}: Readonly<{
  title: string;
  values: ValueExpr[];
  locationType: 'command_argument';
  onChange: (values: ValueExpr[]) => void;
}>) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center">
        <span className="text-[10px] font-medium text-slate-500">{title}</span>
        <button
          type="button"
          className="ml-auto flex size-6 items-center justify-center rounded text-blue-600 hover:bg-blue-50"
          aria-label="添加命令参数"
          onClick={() => onChange([...values, { type: 'literal', value: '' }])}
        >
          <Plus className="size-3.5" aria-hidden="true" />
        </button>
      </div>
      {values.map((value, index) => (
        <div key={index} className="relative">
          <ValueExprFields
            value={value}
            literalLabel={`参数 ${index + 1}`}
            expressionLocation={{ type: locationType, index }}
            onChange={(nextValue) => onChange(values.map((candidate, candidateIndex) => (
              candidateIndex === index ? nextValue : candidate
            )))}
          />
          <button
            type="button"
            className="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            aria-label={`删除参数 ${index + 1}`}
            onClick={() => onChange(values.filter((_, candidateIndex) => candidateIndex !== index))}
          >
            <Trash2 className="size-3" aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}

/** 编辑显式环境变量名和值表达式。 */
function EnvironmentFields({
  bindings,
  onChange,
}: Readonly<{
  bindings: EnvironmentBinding[];
  onChange: (bindings: EnvironmentBinding[]) => void;
}>) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center">
        <span className="text-[10px] font-medium text-slate-500">环境变量</span>
        <button
          type="button"
          className="ml-auto flex size-6 items-center justify-center rounded text-blue-600 hover:bg-blue-50"
          aria-label="添加环境变量"
          onClick={() => onChange([
            ...bindings,
            { name: '', value: { type: 'literal', value: '' } },
          ])}
        >
          <Plus className="size-3.5" aria-hidden="true" />
        </button>
      </div>
      {bindings.map((binding, index) => (
        <div
          key={index}
          className="relative flex flex-col gap-2 rounded-md border border-slate-200 bg-slate-50/60 p-2.5"
        >
          <InspectorField label="变量名称">
            <Input
              aria-label={`环境变量 ${index + 1} 名称`}
              value={binding.name}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onChange(bindings.map((candidate, candidateIndex) => (
                candidateIndex === index
                  ? { ...candidate, name: event.target.value }
                  : candidate
              )))}
            />
          </InspectorField>
          <ValueExprFields
            value={binding.value}
            literalLabel="变量值"
            expressionLocation={{ type: 'command_environment', index }}
            onChange={(value) => onChange(bindings.map((candidate, candidateIndex) => (
              candidateIndex === index ? { ...candidate, value } : candidate
            )))}
          />
          <button
            type="button"
            className="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded text-slate-400 hover:bg-rose-50 hover:text-rose-600"
            aria-label={`删除环境变量 ${index + 1}`}
            onClick={() => onChange(bindings.filter((_, candidateIndex) => candidateIndex !== index))}
          >
            <Trash2 className="size-3" aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}

/** 编辑 stdout/stderr 的显式字节上限。 */
function ByteLimitField({
  label,
  value,
  onChange,
}: Readonly<{
  label: string;
  value: number;
  onChange: (value: number) => void;
}>) {
  return (
    <InspectorField label={label}>
      <Input
        aria-label={label}
        type="number"
        min={1}
        max={16 * 1024 * 1024}
        value={value}
        containerClassName="border-slate-300 bg-white"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </InspectorField>
  );
}

/** 将逗号分隔整数转换成稳定且去重的退出代码列表。 */
function parseExitCodes(source: string): number[] {
  return Array.from(new Set(source
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean)
    .map(Number)
    .filter(Number.isInteger)));
}
