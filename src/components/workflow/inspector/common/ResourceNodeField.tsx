import type {
  ResourceRef,
  WorkflowResourceCatalog,
  WorkflowResourceKind,
  WorkflowResourceOption,
} from '../../../../features/workflow';
import { Select, type SelectOption } from '../../../ui';
import { InspectorField } from '../InspectorControls';

type ResourceNodeFieldProps = Readonly<{
  /** 当前引用需要的资源节点类别。 */
  kind: WorkflowResourceKind;
  /** 当前持久化的逻辑资源引用。 */
  resource: ResourceRef;
  /** 当前消费节点可见的完整资源目录。 */
  catalog: WorkflowResourceCatalog;
  /** 写回固定 session 端口的新资源引用。 */
  onChange: (resource: ResourceRef) => void;
}>;

/** 使用友好节点名称选择应用或浏览器资源，不暴露可编辑内部编号。 */
export function ResourceNodeField({
  kind,
  resource,
  catalog,
  onChange,
}: ResourceNodeFieldProps) {
  const resourceLabel = kind === 'application' ? '应用节点' : '浏览器节点';
  const knownOptions = catalog[kind];
  const selectedOption = knownOptions.find((option) => option.nodeId === resource.producer_node_id);
  /** 已失效引用仍作为禁用项展示，避免属性面板把当前值伪装成未选择。 */
  const missingOption: WorkflowResourceOption | null = resource.producer_node_id && !selectedOption
    ? {
        kind,
        nodeId: resource.producer_node_id,
        nodeLabel: resource.producer_node_id,
        available: false,
        unavailableReason: '节点不存在',
      }
    : null;
  const options: ReadonlyArray<SelectOption<string>> = [
    { value: '', label: `请选择${resourceLabel}` },
    ...knownOptions.map(toSelectOption),
    ...(missingOption ? [toSelectOption(missingOption)] : []),
  ];
  const currentOption = selectedOption ?? missingOption;

  return (
    <InspectorField label={resourceLabel}>
      <div className="min-w-0">
        <Select
          aria-label={resourceLabel}
          value={resource.producer_node_id}
          options={options}
          containerClassName="border-slate-300 bg-white"
          onValueChange={(producerNodeId) => onChange({
            producer_node_id: producerNodeId,
            output_name: 'session',
          })}
        />
        {currentOption && !currentOption.available ? (
          <p className="mt-1.5 text-[10px] leading-4 text-amber-700">
            {currentOption.unavailableReason}
          </p>
        ) : null}
      </div>
    </InspectorField>
  );
}

/** 把领域目录项转换为通用 Select 的展示契约。 */
function toSelectOption(option: WorkflowResourceOption): SelectOption<string> {
  return {
    value: option.nodeId,
    label: option.nodeLabel,
    description: option.available
      ? `内部编号：${option.nodeId}`
      : `${option.nodeId} · ${option.unavailableReason}`,
    disabled: !option.available,
  };
}
