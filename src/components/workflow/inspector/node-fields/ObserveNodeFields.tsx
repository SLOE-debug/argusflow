import type {
  BackendKind,
  ObservationValueType,
  ObserveSpec,
  TargetScope,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { Input, Select } from '../../../ui';
import { AqlFieldSummary } from '../common/AqlFieldSummary';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';

type ObserveNodeFieldsProps = Readonly<{
  nodeId: string;
  observation: ObserveSpec;
  resultType: ObservationValueType;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

const RESULT_TYPE_OPTIONS = [
  { value: 'boolean', label: '判断是或否' },
  { value: 'entities', label: '返回找到的项目' },
  { value: 'records', label: '提取指定信息' },
  { value: 'number', label: '数量' },
] as const;

const SCOPE_OPTIONS = [
  { value: 'current', label: '当前窗口' },
  { value: 'application', label: '指定应用' },
  { value: 'browser', label: '指定浏览器' },
] as const;

const BACKEND_OPTIONS = [
  { value: 'auto', label: '自动选择（推荐）' },
  { value: 'windows_uia', label: 'Windows 控件' },
  { value: 'browser_cdp', label: '网页元素' },
  { value: 'ocr_small', label: '屏幕文字（OCR）' },
] as const;

type ObservationBackendPreset = 'auto' | Extract<
  BackendKind,
  'windows_uia' | 'browser_cdp' | 'ocr_small'
>;

/** 编辑 Observe 的事实范围、AQL 类型提示、后端与有限等待预算。 */
export function ObserveNodeFields({
  nodeId,
  observation,
  resultType,
  onUpdate,
  onOpenEditor,
}: ObserveNodeFieldsProps) {
  const scope = observation.scope;
  const boundedPolicy = observation.policy.mode === 'bounded' ? observation.policy : null;
  const updateObservation = (next: ObserveSpec) => onUpdate((current) => (
    current.kind === 'observe'
      ? { ...current, observation: next, invalid: false }
      : current
  ));
  const target = {
    scope: observation.scope,
    locator: { type: 'query' as const, query: observation.query },
    backend_policy: observation.backend_policy,
  };
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="需要什么结果">
        <Select<ObservationValueType>
          value={resultType}
          options={RESULT_TYPE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(nextResultType) => onUpdate((current) => (
            current.kind === 'observe'
              ? { ...current, resultType: nextResultType, invalid: false }
              : current
          ))}
        />
      </InspectorField>
      <AqlFieldSummary
        query={observation.query}
        target={target}
        onEdit={() => onOpenEditor({ type: 'aql', nodeId })}
      />
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        这里决定节点显示哪些出口。检查规则会在运行前再次确认结果类型。
      </p>
      <InspectorField label="检查范围">
        <Select<TargetScope['type']>
          value={scope.type}
          options={SCOPE_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(type) => updateObservation({
            ...observation,
            scope: createEmptyScope(type),
          })}
        />
      </InspectorField>
      {scope.type !== 'current' ? (
        <InspectorField label={scope.type === 'browser' ? '浏览器节点' : '应用节点'}>
          <Input
            value={scope.resource.producer_node_id}
            containerClassName="border-slate-300 bg-white"
            onChange={(event) => updateObservation({
              ...observation,
              scope: {
                ...scope,
                resource: {
                  ...scope.resource,
                  producer_node_id: event.target.value,
                },
              },
            })}
          />
        </InspectorField>
      ) : null}
      <InspectorField label="检查方式">
        <Select<ObservationBackendPreset>
          value={resolveBackendPreset(observation)}
          options={BACKEND_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(backend) => updateObservation({
            ...observation,
            backend_policy: backend === 'auto'
              ? { allow: [], deny: [], prefer: [] }
              : { allow: [backend], deny: [], prefer: [backend] },
          })}
        />
      </InspectorField>
      <InspectorField label="何时继续">
        <Select<'once' | 'bounded'>
          value={observation.policy.mode}
          options={[
            { value: 'once', label: '检查一次' },
            { value: 'bounded', label: '等待结果出现' },
          ]}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(mode) => updateObservation({
            ...observation,
            policy: mode === 'once'
              ? { mode }
              : { mode, timeout_ms: 5_000, poll_interval_ms: 150 },
          })}
        />
      </InspectorField>
      {boundedPolicy ? (
        <div className="grid grid-cols-2 gap-2">
          <InspectorField label="最长等待（毫秒）">
            <Input
              type="number"
              min={1}
              max={600_000}
              value={boundedPolicy.timeout_ms}
              onChange={(event) => updateObservation({
                ...observation,
                policy: {
                  ...boundedPolicy,
                  timeout_ms: Number(event.target.value),
                },
              })}
            />
          </InspectorField>
          <InspectorField label="检查间隔（毫秒）">
            <Input
              type="number"
              min={1}
              max={60_000}
              value={boundedPolicy.poll_interval_ms}
              onChange={(event) => updateObservation({
                ...observation,
                policy: {
                  ...boundedPolicy,
                  poll_interval_ms: Number(event.target.value),
                },
              })}
            />
          </InspectorField>
        </div>
      ) : null}
    </div>
  );
}

/** 为资源观察建立字段完整的空作用域。 */
function createEmptyScope(type: TargetScope['type']): TargetScope {
  return type === 'current'
    ? { type }
    : { type, resource: { producer_node_id: '', output_name: 'session' } };
}

/** 只有精确单后端策略映射为可编辑预设。 */
function resolveBackendPreset(observation: ObserveSpec): ObservationBackendPreset {
  const { allow, deny, prefer } = observation.backend_policy;
  const backend = allow[0];
  if (allow.length === 1 && deny.length === 0 && prefer[0] === backend
    && (backend === 'windows_uia' || backend === 'browser_cdp' || backend === 'ocr_small')) {
    return backend;
  }
  return 'auto';
}
