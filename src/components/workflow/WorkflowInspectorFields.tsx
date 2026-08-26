import type { WorkflowPermissions } from '../../features/workflow/contracts';
import { Checkbox } from '../ui';

import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
  InspectorSection,
} from './InspectorControls';

type WorkflowInspectorFieldsProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** JSON 变量编辑草稿。 */
  variablesDraft: string;
  /** JSON 变量草稿的即时错误。 */
  variablesError: string | null;
  /** 持久化运行输入声明草稿。 */
  inputDefinitionsDraft: string;
  /** 输入声明草稿错误。 */
  inputDefinitionsError: string | null;
  /** 本次运行的瞬时输入值草稿。 */
  runInputValuesDraft: string;
  /** 本次运行输入值错误。 */
  runInputValuesError: string | null;
  /** Application 与 Command 节点使用的显式能力声明。 */
  permissions: WorkflowPermissions;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 修改 JSON 变量草稿。 */
  onVariablesChange: (draft: string) => void;
  /** 修改运行输入声明。 */
  onInputDefinitionsChange: (draft: string) => void;
  /** 修改本次运行的瞬时输入值。 */
  onRunInputValuesChange: (draft: string) => void;
  /** 修改系统能力声明。 */
  onPermissionsChange: (permissions: WorkflowPermissions) => void;
}>;

/** 工作流级信息、能力、输入和变量设置。 */
export function WorkflowInspectorFields({
  workflowName,
  variablesDraft,
  variablesError,
  inputDefinitionsDraft,
  inputDefinitionsError,
  runInputValuesDraft,
  runInputValuesError,
  permissions,
  onNameChange,
  onVariablesChange,
  onInputDefinitionsChange,
  onRunInputValuesChange,
  onPermissionsChange,
}: WorkflowInspectorFieldsProps) {
  return (
    <>
      <InspectorSection title="基本信息">
        <InspectorField label="流程名称">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={workflowName}
            onChange={(event) => onNameChange(event.target.value)}
          />
        </InspectorField>
        <InspectorField label="流程 ID">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value="workflow_sync_01"
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="系统权限">
        <PermissionToggle
          label="允许 Application 启动应用"
          checked={permissions.application_launch}
          onChange={(application_launch) => onPermissionsChange({
            ...permissions,
            application_launch,
          })}
        />
        <PermissionToggle
          label="允许 Direct 命令"
          checked={permissions.direct_command}
          onChange={(direct_command) => onPermissionsChange({
            ...permissions,
            direct_command,
          })}
        />
        <PermissionToggle
          label="允许 PowerShell"
          checked={permissions.powershell}
          onChange={(powershell) => onPermissionsChange({
            ...permissions,
            powershell,
          })}
        />
        <PermissionToggle
          label="允许 CMD"
          checked={permissions.cmd}
          onChange={(cmd) => onPermissionsChange({ ...permissions, cmd })}
        />
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          每种会创建进程的节点路径都必须拥有对应能力声明。
        </p>
      </InspectorSection>
      <JsonEditorSection
        title="运行输入声明"
        draft={inputDefinitionsDraft}
        error={inputDefinitionsError}
        help="声明随工作流保存；当前仅支持 { key, value_type: 'text' }。"
        onChange={onInputDefinitionsChange}
      />
      <JsonEditorSection
        title="本次运行输入"
        draft={runInputValuesDraft}
        error={runInputValuesError}
        help="值只随本次 run_workflow 调用发送，不写入工作流定义。"
        onChange={onRunInputValuesChange}
      />
      <JsonEditorSection
        title="流程变量"
        draft={variablesDraft}
        error={variablesError}
        help="条件节点使用 RFC 6901 JSON Pointer 读取这里的初始值。"
        onChange={onVariablesChange}
      />
    </>
  );
}

/** 统一渲染带即时错误和格式化操作的 JSON 配置区。 */
function JsonEditorSection({
  title,
  draft,
  error,
  help,
  onChange,
}: Readonly<{
  title: string;
  draft: string;
  error: string | null;
  help: string;
  onChange: (draft: string) => void;
}>) {
  return (
    <InspectorSection title={title}>
      <textarea
        className={`${INSPECTOR_CONTROL_CLASS_NAME} h-[150px] resize-none py-2 font-mono leading-5`}
        spellCheck={false}
        value={draft}
        onChange={(event) => onChange(event.target.value)}
      />
      {error ? (
        <p className="text-[11px] leading-4 text-rose-600">{error}</p>
      ) : (
        <button
          type="button"
          className="flex h-8 items-center justify-center self-start rounded-[4px] border border-slate-300 bg-white px-3 text-[11px] text-slate-600 hover:bg-slate-50"
          onClick={() => onChange(JSON.stringify(JSON.parse(draft), null, 2))}
        >
          格式化 JSON
        </button>
      )}
      <p className={INSPECTOR_HELP_CLASS_NAME}>{help}</p>
    </InspectorSection>
  );
}

/** 统一渲染工作流级布尔权限开关。 */
function PermissionToggle({
  label,
  checked,
  onChange,
}: Readonly<{
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}>) {
  return (
    <label className="flex h-8 items-center gap-2 text-[11px] text-slate-700">
      <Checkbox
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  );
}
