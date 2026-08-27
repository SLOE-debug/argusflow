import type {
  TargetScope,
  TargetLocatorKind,
  UiOperation,
  UiOperationKind,
} from '../../features/workflow/contracts';
import {
  changeBackendPolicy,
  changeSetValue,
  changeTargetLocator,
  changeTargetLocatorKind,
  changeTargetScope,
  changeUiOperationKind,
  resolveBackendPolicyPreset,
  type BackendPolicyPreset,
} from '../../features/workflow/workflowAction';
import { Input, Select } from '../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
import { AqlFieldSummary } from './AqlFieldSummary';
import { ValueExprFields } from './ValueExprFields';
import type { StructuredEditorTarget } from './structuredEditorTarget';

type ActionNodeFieldsProps = Readonly<{
  /** 当前 UI 节点的稳定标识，用于隔离 Monaco 文档。 */
  nodeId: string;
  /** 当前 UI 节点的完整语义操作契约。 */
  operation: UiOperation;
  /** 写回字段完整的新操作。 */
  onChange: (operation: UiOperation) => void;
  /** 请求 Workspace 打开一个结构化文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

const OPERATION_KIND_OPTIONS = [
  { value: 'click', label: '点击' },
  { value: 'set_value', label: '输入文字' },
  { value: 'get_text', label: '读取文本' },
  { value: 'get_value', label: '读取值' },
  { value: 'collect_links', label: '批量读取链接' },
] as const;

const LOCATOR_KIND_OPTIONS = [
  { value: 'query', label: '语义查找（AQL）' },
  { value: 'visual', label: '按画面文字查找' },
  { value: 'coordinate', label: '按屏幕位置' },
] as const;

const SCOPE_OPTIONS = [
  { value: 'current', label: '当前上下文' },
  { value: 'application', label: '应用会话' },
  { value: 'browser', label: '浏览器会话' },
] as const;

const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动选择（推荐）' },
  { value: 'windows_uia', label: 'Windows UIA' },
  { value: 'browser_cdp', label: 'Browser CDP' },
] as const;

const VISUAL_MATCH_OPTIONS = [
  { value: 'exact', label: '完全相等' },
  { value: 'contains', label: '允许包含' },
] as const;

/** 编辑 UI 操作、资源作用域、定位方式和后端偏好。 */
export function ActionNodeFields({
  nodeId,
  operation,
  onChange,
  onOpenEditor,
}: ActionNodeFieldsProps) {
  /** 当前资源作用域的局部不可变快照，供 JSX 回调保留判别联合收窄。 */
  const scope = operation.target.scope;
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="操作">
        <Select<UiOperationKind>
          value={operation.type}
          options={OPERATION_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeUiOperationKind(operation, kind))}
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
      <InspectorField label="应用范围">
        <Select<'current' | 'application' | 'browser'>
          value={scope.type}
          options={SCOPE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => onChange(changeTargetScope(
            operation,
            createEmptyScope(type),
          ))}
        />
      </InspectorField>
      {scope.type !== 'current' ? (
        <div className="flex flex-col gap-2.5 rounded-md border border-blue-100 bg-blue-50/40 p-2.5">
          <InspectorField label={scope.type === 'browser'
            ? '浏览器节点 ID'
            : '应用节点 ID'}>
            <Input
              aria-label="应用资源生产节点 ID"
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
            Runtime 会验证对应资源节点在所有到达路径上先执行。
          </p>
        </div>
      ) : null}
      <InspectorField label="查找目标">
        <Select<TargetLocatorKind>
          value={operation.target.locator.type}
          options={LOCATOR_KIND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(kind) => onChange(changeTargetLocatorKind(operation, kind))}
        />
      </InspectorField>
      {operation.target.locator.type === 'query' ? (
        <QueryTargetFields
          nodeId={nodeId}
          operation={operation}
          locator={operation.target.locator}
          onChange={onChange}
          onOpenEditor={onOpenEditor}
        />
      ) : null}
      {operation.target.locator.type === 'visual' ? (
        <VisualTargetFields
          operation={operation}
          locator={operation.target.locator}
          onChange={onChange}
        />
      ) : null}
      {operation.target.locator.type === 'coordinate' ? (
        <CoordinateTargetFields
          operation={operation}
          locator={operation.target.locator}
          onChange={onChange}
        />
      ) : null}
    </div>
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
  return (
    <>
      <AqlFieldSummary
        query={locator.query}
        target={operation.target}
        onEdit={() => onOpenEditor({ type: 'aql', nodeId })}
      />
      <details className="rounded-md border border-slate-200 bg-slate-50/70 px-2.5 py-2">
        <summary className="cursor-pointer select-none text-[10px] font-medium text-slate-600">
          高级设置
        </summary>
        <div className="mt-2">
          <InspectorField label="执行方式约束">
            <Select<BackendPolicyPreset>
              value={resolveBackendPolicyPreset(operation.target.backend_policy)}
              options={BACKEND_OPTIONS}
              containerClassName="border-slate-300 bg-white"
              onValueChange={(preference) => (
                onChange(changeBackendPolicy(operation, preference))
              )}
            />
          </InspectorField>
        </div>
      </details>
    </>
  );
}

/** 编辑显式 OCR/视觉文字目标。 */
function VisualTargetFields({
  operation,
  locator,
  onChange,
}: Readonly<{
  operation: UiOperation;
  locator: Extract<UiOperation['target']['locator'], { type: 'visual' }>;
  onChange: (operation: UiOperation) => void;
}>) {
  const visualQuery = locator.query;
  return (
    <>
      <InspectorField label="目标文字">
        <Input
          aria-label="视觉目标文字"
          value={visualQuery.text}
          containerClassName="border-slate-300 bg-white"
          onChange={(event) => onChange(changeTargetLocator(operation, {
            type: 'visual',
            query: { ...visualQuery, text: event.target.value },
          }))}
        />
      </InspectorField>
      <InspectorField label="匹配方式">
        <Select<'exact' | 'contains'>
          value={visualQuery.exact ? 'exact' : 'contains'}
          options={VISUAL_MATCH_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(mode) => onChange(changeTargetLocator(operation, {
            type: 'visual',
            query: { ...visualQuery, exact: mode === 'exact' },
          }))}
        />
      </InspectorField>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        视觉文字由 OCR 或 GUI grounding 定位，执行后端固定由 Planner 自动选择。
      </p>
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
        坐标使用 Windows 虚拟屏幕物理像素，仅适合无法提供语义树的最终兜底。
      </p>
    </>
  );
}
