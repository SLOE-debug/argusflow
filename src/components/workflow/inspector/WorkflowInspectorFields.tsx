import type { WorkflowPermissions } from '../../../features/workflow';
import { Button, Checkbox } from '../../ui';

import {
  INSPECTOR_CONTROL_CLASS_NAME,
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
  InspectorSection,
} from './InspectorControls';

type WorkflowInspectorFieldsProps = Readonly<{
  /** 当前工作流名称。 */
  workflowName: string;
  /** Application 与 Command 节点使用的显式能力声明。 */
  permissions: WorkflowPermissions;
  /** 修改工作流名称。 */
  onNameChange: (name: string) => void;
  /** 修改系统能力声明。 */
  onPermissionsChange: (permissions: WorkflowPermissions) => void;
  /** 打开工作流数据面板。 */
  onOpenWorkflowData?: () => void;
}>;

/** 工作流级信息、能力、输入和变量设置。 */
export function WorkflowInspectorFields({
  workflowName,
  permissions,
  onNameChange,
  onPermissionsChange,
  onOpenWorkflowData,
}: WorkflowInspectorFieldsProps) {
  return (
    <>
      <InspectorSection title="基本信息">
        <InspectorField label="工作流名称">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value={workflowName}
            onChange={(event) => onNameChange(event.target.value)}
          />
        </InspectorField>
        <InspectorField label="工作流 ID">
          <input
            className={`${INSPECTOR_CONTROL_CLASS_NAME} h-8`}
            value="workflow_sync_01"
            readOnly
          />
        </InspectorField>
      </InspectorSection>
      <InspectorSection title="运行权限">
        <PermissionToggle
          label="允许启动桌面应用"
          checked={hasPermission(permissions, 'process.application.launch')}
          onChange={(allowed) => onPermissionsChange(changePermission(
            permissions,
            'process.application.launch',
            allowed,
          ))}
        />
        <PermissionToggle
          label="允许直接运行程序"
          checked={hasPermission(permissions, 'process.command.direct')}
          onChange={(allowed) => onPermissionsChange(changePermission(
            permissions,
            'process.command.direct',
            allowed,
          ))}
        />
        <PermissionToggle
          label="允许 PowerShell"
          checked={hasPermission(permissions, 'process.command.powershell')}
          onChange={(allowed) => onPermissionsChange(changePermission(
            permissions,
            'process.command.powershell',
            allowed,
          ))}
        />
        <PermissionToggle
          label="允许 CMD"
          checked={hasPermission(permissions, 'process.command.cmd')}
          onChange={(allowed) => onPermissionsChange(changePermission(
            permissions,
            'process.command.cmd',
            allowed,
          ))}
        />
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          开启后，工作流才能使用对应的程序或命令。
        </p>
      </InspectorSection>
      <InspectorSection title="工作流数据">
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          输入参数、变量和节点输出都在工作流数据面板中管理。
        </p>
        <Button onClick={onOpenWorkflowData}>打开工作流数据</Button>
      </InspectorSection>
    </>
  );
}

/** 判断开放权限集合是否包含指定能力。 */
function hasPermission(permissions: WorkflowPermissions, capability: string): boolean {
  return permissions.allow.includes(capability);
}

/** 以稳定顺序增删一个系统能力授权。 */
function changePermission(
  permissions: WorkflowPermissions,
  capability: string,
  allowed: boolean,
): WorkflowPermissions {
  const allow = allowed
    ? [...new Set([...permissions.allow, capability])]
    : permissions.allow.filter((candidate) => candidate !== capability);
  return { allow: allow.sort() };
}

/** 统一渲染工作流级权限开关。 */
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
