import type {
  ComponentInstance,
  ValueExpr,
} from '../../features/workflow/contracts';
import type { FlowComponentCatalogItem } from '../../features/workflow/componentCatalog';
import type {
  WorkflowNodeData,
  WorkflowNodeUpdater,
} from '../../features/workflow/workflowModel';
import {
  INSPECTOR_HELP_CLASS_NAME,
  InspectorField,
} from './InspectorControls';
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
  const updateComponent = (component: ComponentInstance) => onUpdate((current) => (
    current.kind === 'component'
      ? { ...current, component, invalid: false }
      : current
  ));
  return (
    <div className="flex flex-col gap-2.5">
      <InspectorField label="流程组件">
        <input
          className="h-8 w-full rounded border border-slate-300 bg-slate-50 px-2 text-[11px] text-slate-700"
          value={data.componentName}
          readOnly
        />
      </InspectorField>
      <InspectorField label="使用版本">
        <select
          className="h-8 w-full rounded border border-slate-300 bg-slate-50 px-2 font-mono text-[11px] text-slate-700"
          value={data.component.component_version}
          onChange={(event) => {
            const item = availableVersions.find((candidate) => (
              candidate.definition.version === event.target.value
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
        >
          {availableVersions.map((item) => (
            <option key={item.definition.version} value={item.definition.version}>
              {item.definition.version}
            </option>
          ))}
        </select>
      </InspectorField>
      {Object.entries(data.component.inputs).map(([name, value]) => (
        <div key={name} className="rounded-md border border-violet-100 bg-violet-50/40 p-2.5">
          <ValueExprFields
            value={value}
            literalLabel={`组件输入 ${name}`}
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
        使用的版本不会自动变化；双击节点可查看组件内部流程。
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
