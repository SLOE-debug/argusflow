import type {
  ActionInspectorViewModel,
  ActionLocationViewModel,
  IntentControlRole,
  IntentMatchMode,
  UiExecutionPolicy,
  UiOperation,
  WorkflowResourceCatalog,
  WorkflowResourceKind,
} from '../../../../features/workflow';
import {
  changeActionControlRole,
  changeActionLocation,
  changeActionTargetMatch,
  changeActionTargetText,
  changeActionTargetType,
  changeTargetLocator,
  createTargetWaitPolicy,
  INTENT_CONTROL_ROLE_LABELS,
} from '../../../../features/workflow';
import { Input, Select, type SelectOption } from '../../../ui';
import { AqlEditButton } from '../common/AqlEditButton';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
  InspectorSection,
} from '../InspectorControls';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';
import { TargetStatus } from './TargetStatus';

type TargetSectionProps = Readonly<{
  /** 当前 UI 操作。 */
  operation: UiOperation;
  /** 当前动作的 Intent ViewModel。 */
  viewModel: ActionInspectorViewModel;
  /** 当前节点可以引用的资源。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 定位类型变化时同步更新的等待预算。 */
  execution: UiExecutionPolicy;
  /** 写回新操作。 */
  onOperationChange: (operation: UiOperation) => void;
  /** 写回新执行预算。 */
  onExecutionChange: (execution: UiExecutionPolicy) => void;
  /** 打开当前节点的 AQL 文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
  /** 当前节点 ID。 */
  nodeId: string;
}>;

type EditableTargetType = 'text' | 'control' | 'web' | 'coordinate';
type TargetTypeValue = EditableTargetType | 'focus' | 'advanced';

/** 用户语义目标类型；高级规则只在当前查询无法可逆映射时出现。 */
const TARGET_TYPE_OPTIONS: ReadonlyArray<SelectOption<TargetTypeValue>> = [
  { value: 'text', label: '文字' },
  { value: 'control', label: '控件' },
  { value: 'web', label: '网页元素' },
  { value: 'coordinate', label: '坐标' },
  { value: 'focus', label: '当前焦点', disabled: true },
  {
    value: 'advanced',
    label: '高级规则',
    description: '此查询包含只能在 AQL 编辑器中修改的代码',
    disabled: true,
  },
];

/** 文字和控件目标共用的用户语言匹配方式。 */
const MATCH_OPTIONS = [
  { value: 'exact', label: '完全匹配' },
  { value: 'contains', label: '包含' },
  { value: 'starts_with', label: '开头是' },
  { value: 'ends_with', label: '结尾是' },
  { value: 'regex', label: '正则表达式' },
] as const;

/** 控件目标可以直接选择的稳定角色。 */
const CONTROL_ROLE_OPTIONS = Object.entries(INTENT_CONTROL_ROLE_LABELS).map(([value, label]) => ({
  value: value as IntentControlRole,
  label,
}));

/** 只回答“在哪里、找什么”的目标区块。 */
export function TargetSection({
  operation,
  viewModel,
  resourceCatalog,
  execution,
  onOperationChange,
  onExecutionChange,
  onOpenEditor,
  nodeId,
}: TargetSectionProps) {
  const locationOptions = buildLocationOptions(resourceCatalog, viewModel.location);
  const targetType = viewModel.target.type;
  return (
    <InspectorSection
      title="找什么"
      action={operation.target.locator.type === 'query' ? (
        <AqlEditButton onEdit={() => onOpenEditor({ type: 'aql', nodeId })} />
      ) : null}
    >
      <InspectorField label="应用 / 窗口">
        <Select<ActionLocationViewModel['value']>
          aria-label="应用 / 窗口"
          value={viewModel.location.value}
          options={locationOptions}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(value) => onOperationChange(changeActionLocation(operation, value))}
        />
        {viewModel.location.sourceLabel ? (
          <p className="mt-1.5 text-[10px] leading-4 text-slate-500">
            来源：{viewModel.location.sourceLabel}
          </p>
        ) : null}
        {viewModel.location.unavailableReason ? (
          <p className="mt-1 text-[10px] leading-4 text-amber-700">
            {viewModel.location.unavailableReason}
          </p>
        ) : null}
      </InspectorField>
      <InspectorField label="目标类型">
        <Select<TargetTypeValue>
          aria-label="目标类型"
          value={targetType}
          options={TARGET_TYPE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          disabled={targetType === 'focus'}
          onValueChange={(value) => {
            if (value === 'focus' || value === 'advanced') return;
            const nextOperation = changeActionTargetType(operation, value);
            onOperationChange(nextOperation);
            if (nextOperation.target.locator.type !== operation.target.locator.type) {
              onExecutionChange({
                ...execution,
                target_wait: createTargetWaitPolicy(nextOperation.target.locator.type),
              });
            }
          }}
        />
      </InspectorField>
      <TargetEditor
        operation={operation}
        target={viewModel.target}
        onChange={onOperationChange}
      />
      <TargetStatus status={viewModel.targetStatus} />
    </InspectorSection>
  );
}

/** 按目标判别联合渲染直接可见的核心条件。 */
function TargetEditor({
  operation,
  target,
  onChange,
}: Readonly<{
  operation: UiOperation;
  target: ActionInspectorViewModel['target'];
  onChange: (operation: UiOperation) => void;
}>) {
  switch (target.type) {
    case 'text':
      return (
        <>
          <TargetTextField
            label="文字"
            value={target.value.text}
            editable={target.editable}
            onChange={(value) => onChange(changeActionTargetText(operation, value))}
          />
          <MatchField
            value={target.match}
            disabled={!target.editable}
            onChange={(match) => onChange(changeActionTargetMatch(operation, match))}
          />
          {target.hasMoreConditions ? <MoreConditionsHelp /> : null}
        </>
      );
    case 'control':
      return (
        <>
          <InspectorField label="控件类型">
            <Select<IntentControlRole>
              aria-label="控件类型"
              value={target.role}
              options={CONTROL_ROLE_OPTIONS}
              containerClassName="border-slate-300 bg-white"
              onValueChange={(role) => onChange(changeActionControlRole(operation, role))}
            />
          </InspectorField>
          <TargetTextField
            label="名称"
            value={target.value.text}
            editable={target.editable}
            onChange={(value) => onChange(changeActionTargetText(operation, value))}
          />
          <MatchField
            value={target.match}
            disabled={!target.editable}
            onChange={(match) => onChange(changeActionTargetMatch(operation, match))}
          />
          {target.hasMoreConditions ? <MoreConditionsHelp /> : null}
        </>
      );
    case 'web':
      return (
        <TargetTextField
          label="CSS 选择器"
          value={target.selector}
          editable={target.editable}
          onChange={(value) => onChange(changeActionTargetText(operation, value))}
        />
      );
    case 'coordinate': {
      const locator = operation.target.locator;
      if (locator.type !== 'coordinate') return null;
      return (
        <>
          <CoordinateField
            axis="x"
            value={target.x}
            onChange={(value) => onChange(changeTargetLocator(operation, {
              ...locator,
              point: { ...locator.point, x: value },
            }))}
          />
          <CoordinateField
            axis="y"
            value={target.y}
            onChange={(value) => onChange(changeTargetLocator(operation, {
              ...locator,
              point: { ...locator.point, y: value },
            }))}
          />
          <p className={INSPECTOR_HELP_CLASS_NAME}>窗口移动后，屏幕坐标可能失效。</p>
        </>
      );
    }
    case 'focus':
      return (
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          动作会发送到当前选中的输入位置；运行前会先切换到指定应用。
        </p>
      );
    case 'advanced':
      return (
        <>
          <InspectorField label="目标内容">
            <Input
              aria-label="目标内容"
              value={target.description}
              readOnly
              containerClassName="border-slate-300 bg-slate-50"
            />
          </InspectorField>
          <p className={INSPECTOR_HELP_CLASS_NAME}>
            此查询包含基础表单无法显示的 AQL 代码。更换目标类型会覆盖这些代码。
          </p>
        </>
      );
  }
}

/** 文字内容字段在绑定运行输入时保持可见但只读。 */
function TargetTextField({
  label,
  value,
  editable,
  onChange,
}: Readonly<{
  label: string;
  value: string;
  editable: boolean;
  onChange: (value: string) => void;
}>) {
  return (
    <InspectorField label={label}>
      <Input
        aria-label={label}
        value={value}
        readOnly={!editable}
        containerClassName={`border-slate-300 ${editable ? 'bg-white' : 'bg-slate-50'}`}
        onChange={(event) => onChange(event.target.value)}
      />
    </InspectorField>
  );
}

/** 文字与控件名称共享匹配方式。 */
function MatchField({
  value,
  disabled,
  onChange,
}: Readonly<{
  value: IntentMatchMode;
  disabled: boolean;
  onChange: (value: IntentMatchMode) => void;
}>) {
  return (
    <InspectorField label="匹配">
      <Select<IntentMatchMode>
        aria-label="匹配"
        value={value}
        options={MATCH_OPTIONS}
        disabled={disabled}
        containerClassName="border-slate-300 bg-white"
        onValueChange={onChange}
      />
    </InspectorField>
  );
}

/** 坐标字段保留显式轴名称与数字语义。 */
function CoordinateField({
  axis,
  value,
  onChange,
}: Readonly<{
  axis: 'x' | 'y';
  value: number;
  onChange: (value: number) => void;
}>) {
  const label = axis === 'x' ? '屏幕 X' : '屏幕 Y';
  return (
    <InspectorField label={label}>
      <Input
        aria-label={`${label} 坐标`}
        type="number"
        value={value}
        containerClassName="border-slate-300 bg-white"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </InspectorField>
  );
}

/** 提醒用户当前目标仍有未铺开的稳定条件。 */
function MoreConditionsHelp() {
  return (
    <p className={INSPECTOR_HELP_CLASS_NAME}>
      此目标还包含未显示的条件。可在 AQL 编辑器中查看或修改。
    </p>
  );
}

/** 把资源目录转换为应用优先、来源次要的统一位置选项。 */
function buildLocationOptions(
  catalog: WorkflowResourceCatalog,
  current: ActionLocationViewModel,
): ReadonlyArray<SelectOption<ActionLocationViewModel['value']>> {
  const options: Array<SelectOption<ActionLocationViewModel['value']>> = [
    { value: 'current', label: '当前窗口' },
    ...resourceOptions('application', catalog.application),
    ...resourceOptions('browser', catalog.browser),
  ];
  if (!options.some(({ value }) => value === current.value)) {
    options.push({
      value: current.value,
      label: current.label,
      description: current.unavailableReason ?? undefined,
      disabled: true,
    });
  }
  return options;
}

/** 把一种资源节点转换成带来源提示的选择项。 */
function resourceOptions(
  kind: WorkflowResourceKind,
  resources: WorkflowResourceCatalog[WorkflowResourceKind],
): ReadonlyArray<SelectOption<ActionLocationViewModel['value']>> {
  return resources.map((resource) => ({
    value: `${kind}:${resource.nodeId}`,
    label: resource.resourceLabel,
    description: resource.available
      ? `来源：${resource.nodeLabel}`
      : `${resource.nodeLabel} · ${resource.unavailableReason}`,
    disabled: !resource.available,
  }));
}
