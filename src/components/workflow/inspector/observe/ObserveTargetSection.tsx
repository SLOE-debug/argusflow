import type {
  ObservationValueType,
  ObserveSpec,
  TargetScope,
  WorkflowResourceCatalog,
  WorkflowResourceOption,
} from '../../../../features/workflow';
import { Select, type SelectOption } from '../../../ui';
import { AqlEditButton } from '../common/AqlEditButton';
import { InspectorField, InspectorSection } from '../InspectorControls';
import type { StructuredEditorTarget } from '../../workspace/dock/structuredEditorTarget';

type ObservationLocationValue =
  | 'current'
  | `application:${string}`
  | `browser:${string}`;

type ObserveTargetSectionProps = Readonly<{
  /** 当前节点稳定标识，用于打开独立 AQL 文档。 */
  nodeId: string;
  /** 当前观察事实来源与查询。 */
  observation: ObserveSpec;
  /** 查询的用户可见返回类型。 */
  resultType: ObservationValueType;
  /** 当前节点可引用的应用和浏览器。 */
  resourceCatalog: WorkflowResourceCatalog;
  /** 写回完整观察契约。 */
  onObservationChange: (observation: ObserveSpec) => void;
  /** 写回返回类型。 */
  onResultTypeChange: (resultType: ObservationValueType) => void;
  /** 打开当前节点的 AQL 文档。 */
  onOpenEditor: (target: StructuredEditorTarget) => void;
}>;

/** 返回结果使用用户语言，不暴露 AQL 顶层类型名称。 */
export const OBSERVATION_RESULT_OPTIONS = [
  { value: 'boolean', label: '是否找到' },
  { value: 'entities', label: '找到的目标' },
  { value: 'records', label: '提取的信息' },
  { value: 'number', label: '目标数量' },
] as const;

/** 检查节点的主路径只回答“在哪里检查、返回什么”。 */
export function ObserveTargetSection({
  nodeId,
  observation,
  resultType,
  resourceCatalog,
  onObservationChange,
  onResultTypeChange,
  onOpenEditor,
}: ObserveTargetSectionProps) {
  const locationValue = scopeToLocationValue(observation.scope);
  const locationOptions = buildLocationOptions(resourceCatalog, locationValue);
  const selectedResource = findSelectedResource(resourceCatalog, locationValue);
  return (
    <InspectorSection
      title="检查什么"
      action={(
        <AqlEditButton onEdit={() => onOpenEditor({ type: 'aql', nodeId })} />
      )}
    >
      <InspectorField label="返回结果">
        <Select<ObservationValueType>
          aria-label="返回结果"
          value={resultType}
          options={OBSERVATION_RESULT_OPTIONS}
          containerClassName="border-slate-300 bg-white"
          onValueChange={onResultTypeChange}
        />
      </InspectorField>
      <InspectorField label="应用 / 窗口">
        <Select<ObservationLocationValue>
          aria-label="应用 / 窗口"
          value={locationValue}
          options={locationOptions}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(value) => onObservationChange({
            ...observation,
            scope: locationValueToScope(value),
          })}
        />
        {selectedResource?.available ? (
          <p className="mt-1 text-[10px] leading-4 text-slate-500">
            来源：{selectedResource.nodeLabel}
          </p>
        ) : null}
        {selectedResource && !selectedResource.available ? (
          <p className="mt-1 text-[10px] leading-4 text-amber-700">
            {selectedResource.unavailableReason}
          </p>
        ) : null}
      </InspectorField>
    </InspectorSection>
  );
}

/** 把领域作用域压缩成单一位置选择器的稳定值。 */
function scopeToLocationValue(scope: TargetScope): ObservationLocationValue {
  switch (scope.type) {
    case 'current':
      return 'current';
    case 'application':
      return `application:${scope.resource.producer_node_id}`;
    case 'browser':
      return `browser:${scope.resource.producer_node_id}`;
  }
}

/** 选择器只产生声明过的判别联合，因此可直接构造字段完整的作用域。 */
function locationValueToScope(value: ObservationLocationValue): TargetScope {
  if (value === 'current') return { type: 'current' };
  if (value.startsWith('application:')) {
    return {
      type: 'application',
      resource: {
        producer_node_id: value.slice('application:'.length),
        output_name: 'session',
      },
    };
  }
  return {
    type: 'browser',
    resource: {
      producer_node_id: value.slice('browser:'.length),
      output_name: 'session',
    },
  };
}

/** 应用与浏览器共用一个位置目录，避免先选范围再选节点的重复步骤。 */
function buildLocationOptions(
  catalog: WorkflowResourceCatalog,
  currentValue: ObservationLocationValue,
): ReadonlyArray<SelectOption<ObservationLocationValue>> {
  const options: Array<SelectOption<ObservationLocationValue>> = [
    { value: 'current', label: '当前窗口' },
    ...catalog.application.map((resource) => resourceToOption('application', resource)),
    ...catalog.browser.map((resource) => resourceToOption('browser', resource)),
  ];
  if (!options.some(({ value }) => value === currentValue)) {
    options.push({
      value: currentValue,
      label: currentValue.split(':')[1] ?? '引用不存在',
      description: '引用的资源节点不存在',
      disabled: true,
    });
  }
  return options;
}

/** 资源选项优先展示真实应用名，来源节点只作为辅助说明。 */
function resourceToOption(
  kind: 'application' | 'browser',
  resource: WorkflowResourceOption,
): SelectOption<ObservationLocationValue> {
  return {
    value: `${kind}:${resource.nodeId}`,
    label: resource.resourceLabel,
    description: resource.available
      ? `来源：${resource.nodeLabel}`
      : `${resource.nodeLabel} · ${resource.unavailableReason}`,
    disabled: !resource.available,
  };
}

/** 返回当前位置对应的资源状态，供字段下方解释来源或失效原因。 */
function findSelectedResource(
  catalog: WorkflowResourceCatalog,
  value: ObservationLocationValue,
): WorkflowResourceOption | null {
  if (value === 'current') return null;
  const kind = value.startsWith('application:') ? 'application' : 'browser';
  const nodeId = value.slice(`${kind}:`.length);
  return catalog[kind].find((resource) => resource.nodeId === nodeId) ?? {
    kind,
    nodeId,
    nodeLabel: nodeId,
    resourceLabel: nodeId,
    available: false,
    unavailableReason: '引用的资源节点不存在',
  };
}
