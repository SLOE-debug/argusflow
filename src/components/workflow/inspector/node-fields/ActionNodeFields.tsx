import type {
  TargetScope,
  TargetLocatorKind,
  UiExecutionPolicy,
  UiOperation,
  UiOperationKind,
} from '../../../../features/workflow';
import {
  changeBackendPolicy,
  changeKeyChord,
  changeSetValue,
  changeTargetLocator,
  changeTargetLocatorKind,
  changeTargetScope,
  changeUiOperationKind,
  changeTypeText,
  createTargetWaitPolicy,
  resolveBackendPolicyPreset,
  type BackendPolicyPreset,
} from '../../../../features/workflow';
import { Checkbox, Input, Select } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';
import { AqlFieldSummary } from '../common/AqlFieldSummary';
import { ValueExprFields } from './ValueExprFields';
import { ExtractNodeFields } from './ExtractNodeFields';
import { KeyboardChordFields } from './KeyboardChordFields';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';

type ActionNodeFieldsProps = Readonly<{
  /** 当前 UI 节点的稳定标识，用于隔离 Monaco 文档。 */
  nodeId: string;
  /** 当前 UI 节点的完整语义操作契约。 */
  operation: UiOperation;
  /** 与目标定位语义分离的节点执行预算。 */
  execution: UiExecutionPolicy;
  /** 写回字段完整的新操作。 */
  onChange: (operation: UiOperation) => void;
  /** 写回字段完整的新执行策略。 */
  onExecutionChange: (execution: UiExecutionPolicy) => void;
  /** 请求 Workspace 打开一个结构化文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

const OPERATION_KIND_OPTIONS = [
  { value: 'click', label: '点击' },
  { value: 'set_value', label: '输入文字' },
  { value: 'press_key', label: '按键' },
  { value: 'type_text', label: '物理输入文字' },
  { value: 'get_text', label: '读取文字' },
  { value: 'get_value', label: '读取控件值' },
  { value: 'extract', label: '读取数据' },
] as const;

const LOCATOR_KIND_OPTIONS = [
  { value: 'query', label: 'AQL 查询' },
  { value: 'coordinate', label: '屏幕坐标' },
  { value: 'focused', label: '当前焦点' },
] as const;

const SCOPE_OPTIONS = [
  { value: 'current', label: '当前窗口' },
  { value: 'application', label: '指定应用' },
  { value: 'browser', label: '指定浏览器' },
] as const;

const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动选择（推荐）' },
  { value: 'windows_uia', label: 'Windows UI 自动化' },
  { value: 'browser_cdp', label: '浏览器自动化' },
  { value: 'ocr_small', label: '画面文字（OCR）' },
  { value: 'send_input', label: '键盘输入' },
] as const;

/** 只有会改变界面的物理输入动作才允许保留视觉新事实后置条件。 */
function acceptsVisualPostcondition(operation: UiOperation): boolean {
  return operation.type === 'press_key' || operation.type === 'type_text';
}

/** 编辑 UI 操作、资源作用域、定位方式和后端偏好。 */
export function ActionNodeFields({
  nodeId,
  operation,
  execution,
  onChange,
  onExecutionChange,
  onOpenEditor,
}: ActionNodeFieldsProps) {
  /** 当前资源作用域的局部不可变快照，供 JSX 回调保留判别联合收窄。 */
  const scope = operation.target.scope;
  /** 当前作用域对应的资源节点名称，避免把内部 Resource 概念直接展示给用户。 */
  const resourceLabel = scope.type === 'browser' ? '浏览器节点' : '应用节点';
  /** 告诉用户资源节点和当前操作的执行顺序。 */
  const resourceHelp = scope.type === 'browser'
    ? '请先运行打开浏览器的节点，再执行当前操作。'
    : '请先运行打开应用的节点，再执行当前操作。';
  /** 键盘动作直接使用当前焦点，不显示无效的元素定位配置。 */
  const usesKeyboardFocus = operation.type === 'press_key' || operation.type === 'type_text';
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="操作">
        <Select<UiOperationKind>
          value={operation.type}
          options={OPERATION_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => {
            const nextOperation = changeUiOperationKind(operation, kind);
            onChange(nextOperation);
            const locatorChanged = nextOperation.target.locator.type !== operation.target.locator.type;
            if (locatorChanged || !acceptsVisualPostcondition(nextOperation)) {
              onExecutionChange({
                ...execution,
                target_wait: locatorChanged
                  ? createTargetWaitPolicy(nextOperation.target.locator.type)
                  : execution.target_wait,
                postcondition: acceptsVisualPostcondition(nextOperation)
                  ? execution.postcondition
                  : null,
              });
            }
          }}
        />
      </InspectorField>
      {operation.type === 'set_value' ? (
        <ValueExprFields
          value={operation.value}
          literalLabel="输入内容"
          expressionLocation={{ type: 'ui_set_value' }}
          onChange={(value) => onChange(changeSetValue(operation, value))}
        />
      ) : null}
      {operation.type === 'type_text' ? (
        <ValueExprFields
          value={operation.value}
          literalLabel="输入内容"
          expressionLocation={{ type: 'ui_type_text' }}
          onChange={(value) => onChange(changeTypeText(operation, value))}
        />
      ) : null}
      {operation.type === 'press_key' ? (
        <KeyboardChordFields
          chord={operation.chord}
          onChange={(chord) => onChange(changeKeyChord(operation, chord))}
        />
      ) : null}
      {operation.type === 'extract' ? (
        <ExtractNodeFields
          operation={operation}
          onChange={onChange}
        />
      ) : null}
      <InspectorField label="操作范围">
        <Select<'current' | 'application' | 'browser'>
          value={scope.type}
          options={usesKeyboardFocus
            ? SCOPE_OPTIONS.filter(({ value }) => value !== 'browser')
            : SCOPE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => onChange(changeTargetScope(
            operation,
            createEmptyScope(type),
          ))}
        />
      </InspectorField>
      {scope.type !== 'current' ? (
        <div className="flex flex-col gap-2.5 rounded-md border border-blue-100 bg-blue-50/40 p-2.5">
          <InspectorField label={resourceLabel}>
            <Input
              aria-label={resourceLabel}
              value={scope.resource.producer_node_id}
              containerClassName="border-slate-300 bg-white"
              onChange={(event) => onChange(changeTargetScope(operation, {
                ...scope,
                resource: {
                  ...scope.resource,
                  producer_node_id: event.target.value,
                },
              }))}
            />
          </InspectorField>
          <p className={INSPECTOR_HELP_CLASS_NAME}>
            {resourceHelp}
          </p>
        </div>
      ) : null}
      {usesKeyboardFocus ? (
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          输入会发送到指定应用窗口的当前焦点；系统会先核对并激活该窗口。
        </p>
      ) : (
        <InspectorField label="查找方式">
          <Select<TargetLocatorKind>
            value={operation.target.locator.type}
            options={LOCATOR_KIND_OPTIONS.filter(({ value }) => value !== 'focused')}
            containerClassName="border-slate-300 bg-white"
            onValueChange={(kind) => {
              onChange(changeTargetLocatorKind(operation, kind));
              onExecutionChange({
                ...execution,
                target_wait: createTargetWaitPolicy(kind),
              });
            }}
          />
        </InspectorField>
      )}
      {operation.target.locator.type === 'query' ? (
        <QueryTargetFields
          nodeId={nodeId}
          operation={operation}
          locator={operation.target.locator}
          onChange={onChange}
          onOpenEditor={onOpenEditor}
        />
      ) : null}
      {operation.target.locator.type === 'coordinate' ? (
        <CoordinateTargetFields
          operation={operation}
          locator={operation.target.locator}
          onChange={onChange}
        />
      ) : null}
      {operation.target.locator.type === 'query' ? (
        <TargetWaitFields
          execution={execution}
          locatorKind={operation.target.locator.type}
          onChange={onExecutionChange}
        />
      ) : null}
      {execution.postcondition !== null ? (
        <PostconditionWaitFields
          execution={execution}
          onChange={onExecutionChange}
        />
      ) : null}
    </div>
  );
}

/** 编辑视觉后置条件自己的观察预算，避免与动作前目标等待共享截止时间。 */
function PostconditionWaitFields({
  execution,
  onChange,
}: Readonly<{
  execution: UiExecutionPolicy;
  onChange: (execution: UiExecutionPolicy) => void;
}>) {
  const policy = execution.postcondition_wait;
  return (
    <div className="rounded-md border border-amber-200 bg-amber-50/60 px-2.5 py-2">
      <InspectorField label="发送后观察超时（毫秒）">
        <Input
          aria-label="后置条件观察超时时间"
          type="number"
          min={1}
          max={600_000}
          value={policy.timeout_ms}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...execution,
            postcondition_wait: {
              ...policy,
              mode: 'bounded',
              timeout_ms: Number(event.target.value),
            },
          })}
        />
      </InspectorField>
      <InspectorField label="发送后检查间隔（毫秒）">
        <Input
          aria-label="后置条件观察轮询间隔"
          type="number"
          min={1}
          max={60_000}
          value={policy.poll_interval_ms}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange({
            ...execution,
            postcondition_wait: {
              ...policy,
              mode: 'bounded',
              poll_interval_ms: Number(event.target.value),
            },
          })}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        发送后会在此预算内重复观察，未确认时不会自动重发。
      </p>
    </div>
  );
}

/** 编辑 UI 节点自己的目标就绪预算，不复制 operation 中的 selector。 */
function TargetWaitFields({
  execution,
  locatorKind,
  onChange,
}: Readonly<{
  execution: UiExecutionPolicy;
  locatorKind: Exclude<TargetLocatorKind, 'coordinate'>;
  onChange: (execution: UiExecutionPolicy) => void;
}>) {
  const policy = execution.target_wait;
  const enabled = policy.mode === 'bounded';
  return (
    <details className="rounded-md border border-slate-200 bg-slate-50/70 px-2.5 py-2">
      <summary className="cursor-pointer select-none text-[10px] font-medium text-slate-600">
        等待目标
      </summary>
      <div className="mt-2 flex flex-col gap-2.5">
        <label className="flex items-center gap-2 text-[11px] text-slate-700">
          <Checkbox
            aria-label="找不到目标时自动等待"
            checked={enabled}
            onChange={(event) => onChange({
              ...execution,
              target_wait: event.target.checked
                ? createTargetWaitPolicy(locatorKind)
                : { mode: 'none', timeout_ms: 0, poll_interval_ms: 0 },
            })}
          />
          找不到目标时自动等待
        </label>
        {enabled ? (
          <>
            <InspectorField label="等待超时（毫秒）">
              <Input
                aria-label="目标等待超时时间"
                type="number"
                min={1}
                max={600_000}
                value={policy.timeout_ms}
                containerClassName="border-slate-300 bg-white"
                onChange={(event) => onChange({
                  ...execution,
                  target_wait: {
                    ...policy,
                    timeout_ms: Number(event.target.value),
                  },
                })}
              />
            </InspectorField>
            <InspectorField label="检查间隔（毫秒）">
              <Input
                aria-label="目标等待轮询间隔"
                type="number"
                min={1}
                max={60_000}
                value={policy.poll_interval_ms}
                containerClassName="border-slate-300 bg-white"
                onChange={(event) => onChange({
                  ...execution,
                  target_wait: {
                    ...policy,
                    poll_interval_ms: Number(event.target.value),
                  },
                })}
              />
            </InspectorField>
          </>
        ) : null}
        <p className={INSPECTOR_HELP_CLASS_NAME}>
          只在暂时找不到目标时等待；目标不明确或无法执行时会立即停止。
        </p>
      </div>
    </details>
  );
}

/** 为资源作用域建立字段完整的判别联合。 */
function createEmptyScope(type: TargetScope['type']): TargetScope {
  if (type === 'current') {
    return { type };
  }
  return {
    type,
    resource: {
      producer_node_id: '',
      output_name: 'session',
    },
  };
}

/** 编辑 AQL 目标及其高级后端约束。 */
function QueryTargetFields({
  nodeId,
  operation,
  locator,
  onChange,
  onOpenEditor,
}: Readonly<{
  nodeId: string;
  operation: UiOperation;
  locator: Extract<UiOperation['target']['locator'], { type: 'query' }>;
  onChange: (operation: UiOperation) => void;
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>) {
  /** OCR 只暴露文字事实；其它操作不展示无法执行的后端选项。 */
  const acceptsOcr = operation.type === 'click'
    || operation.type === 'get_text'
    || operation.type === 'extract' && operation.fields.every((field) => (
      field.source.type === 'text' || field.source.type === 'name'
    ));
  /** 当前后端预设只计算一次，保证选择器与帮助文案使用同一状态。 */
  const backendPreset = resolveBackendPolicyPreset(operation.target.backend_policy);
  return (
    <>
      <AqlFieldSummary
        query={locator.query}
        target={operation.target}
        onEdit={() => onOpenEditor({ type: 'aql', nodeId })}
      />
      <details className="rounded-md border border-slate-200 bg-slate-50/70 px-2.5 py-2">
        <summary className="cursor-pointer select-none text-[10px] font-medium text-slate-600">
          更多设置
        </summary>
        <div className="mt-2">
          <InspectorField label="执行方式">
            <Select<BackendPolicyPreset>
              value={backendPreset}
              options={acceptsOcr
                ? BACKEND_OPTIONS
                : BACKEND_OPTIONS.filter(({ value }) => value !== 'ocr_small')}
              containerClassName="border-slate-300 bg-white"
              onValueChange={(preference) => (
                onChange(changeBackendPolicy(operation, preference))
              )}
            />
          </InspectorField>
          {backendPreset === 'ocr_small' ? (
            <p className={`${INSPECTOR_HELP_CLASS_NAME} mt-1`}>
              OCR 仅查询 text(...) 文字节点；点击会使用命中边界内的安全点。
            </p>
          ) : null}
        </div>
      </details>
    </>
  );
}

/** 编辑 Windows 虚拟屏幕中的物理像素坐标。 */
function CoordinateTargetFields({
  operation,
  locator,
  onChange,
}: Readonly<{
  operation: UiOperation;
  locator: Extract<UiOperation['target']['locator'], { type: 'coordinate' }>;
  onChange: (operation: UiOperation) => void;
}>) {
  const point = locator.point;
  const updatePoint = (axis: 'x' | 'y', value: number) => {
    onChange(changeTargetLocator(operation, {
      type: 'coordinate',
      point: { ...point, [axis]: value },
    }));
  };
  return (
    <>
      <InspectorField label="屏幕 X">
        <Input
          aria-label="屏幕 X 坐标"
          type="number"
          value={point.x}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => updatePoint('x', Number(event.target.value))}
        />
      </InspectorField>
      <InspectorField label="屏幕 Y">
        <Input
          aria-label="屏幕 Y 坐标"
          type="number"
          value={point.y}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => updatePoint('y', Number(event.target.value))}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        只有其他方式找不到目标时，才建议使用屏幕坐标。
      </p>
    </>
  );
}
