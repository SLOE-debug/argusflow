import type {
  ComponentInstance,
  ValueExpr,
} from '../../../../features/workflow';
import type { FlowComponentCatalogItem } from '../../../../features/workflow';
import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../../../features/workflow';
import { Select } from '../../../ui';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from '../InspectorControls';
import { ValueExprFields } from './ValueExprFields';

type ComponentData = Extract<WorkflowNodeData, { kind: 'component' }>;

type ComponentNodeFieldsProps = Readonly<{
  data: ComponentData;
  componentCatalog: ReadonlyArray<FlowComponentCatalogItem>;
  onUpdate: (updater: WorkflowNodeUpdater) => void;
}>;

/** 编辑版本锁定组件实例的显式值输入。 */
export function ComponentNodeFields({
  data,
  componentCatalog,
  onUpdate,
}: ComponentNodeFieldsProps) {
  const availableVersions = componentCatalog.filter((item) => (
    item.definition.id === data.component.component_id
  ));
  /** 当前组件可显式切换的精确版本清单。 */
  const versionOptions = availableVersions.map((item) => ({
    value: item.definition.version,
    label: item.definition.version,
  }));
  const updateComponent = (component: ComponentInstance) => onUpdate((current) => (
    current.kind === 'component'
      ? { ...current, component, invalid: false }
      : current
  ));
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="组合步骤">
        <input
          className="h-8 w-full rounded border border-slate-300 bg-slate-50 px-2 text-[11px] text-slate-700"
          value={data.componentName}
          readOnly
        />
      </InspectorField>
      <InspectorField label="使用版本">
        <Select<string>
          value={data.component.component_version}
          options={versionOptions}
          className="font-mono text-[11px]"
          containerClassName="rounded border-slate-300 bg-slate-50"
          onValueChange={(version) => {
            const item = availableVersions.find((candidate) => (
              candidate.definition.version === version
            ));
            if (!item) return;
            const inputs = resolveVersionInputs(data.component.inputs, item);
            if (!inputs) return;
            onUpdate((current) => current.kind === 'component'
              ? {
                  ...current,
                  label: item.title,
                  componentName: item.definition.name,
                  componentOutputs: item.definition.outputs,
                  componentDefinition: item.definition,
                  component: {
                    component_id: item.definition.id,
                    component_version: item.definition.version,
                    inputs,
                  },
                  invalid: false,
                }
              : current);
          }}
        />
      </InspectorField>
      {Object.entries(data.component.inputs).map(([name, value]) => (
        <div key={name} className="rounded-md border border-violet-100 bg-violet-50/40 p-2.5">
          <ValueExprFields
            value={value}
            literalLabel={name}
            expressionLocation={{ type: 'component_input', name }}
            onChange={(nextValue) => updateComponent({
              ...data.component,
              inputs: { ...data.component.inputs, [name]: nextValue },
            })}
          />
        </div>
      ))}
      <div className="rounded-md border border-slate-200 bg-slate-50 p-2.5">
        <p className="text-[10px] font-semibold text-slate-600">输出</p>
        <p className="mt-1 font-mono text-[10px] text-slate-500">
          {data.componentOutputs.map((output) => output.name).join(', ') || '暂无'}
        </p>
      </div>
      <p className={INSPECTOR_HELP_CLASS_NAME}>
        版本不会自动更新。双击节点可以查看其中的步骤。
      </p>
    </div>
  );
}

/** 显式升级时按新定义端口对齐输入，并保留同名的现有绑定。 */
function resolveVersionInputs(
  current: Readonly<Record<string, ValueExpr>>,
  item: FlowComponentCatalogItem,
): Readonly<Record<string, ValueExpr>> | null {
  const inputs: Record<string, ValueExpr> = {};
  for (const input of item.definition.inputs) {
    const value = current[input.key] ?? item.defaultInputs[input.key];
    if (!value) return null;
    inputs[input.key] = value;
  }
  return inputs;
}
