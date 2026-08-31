import type {
  ComponentInstance,
  FlowComponentDefinition,
  ValueOutputDescriptor,
} from '../model/contracts';

/** Studio 可创建的版本锁定组件目录项。 */
export type FlowComponentCatalogItem = Readonly<{
  title: string;
  description: string;
  definition: FlowComponentDefinition;
  defaultInputs: ComponentInstance['inputs'];
  valueOutputs: ReadonlyArray<ValueOutputDescriptor>;
}>;

/** 内置目录保持为空；完整应用示例直接在默认画布上展示。 */
export const FLOW_COMPONENT_CATALOG = [] as const satisfies ReadonlyArray<FlowComponentCatalogItem>;

/** 通过稳定 ID 和精确版本查找当前工作区目录项。 */
export function findFlowComponent(
  componentId: string,
  componentVersion: string,
  catalog: ReadonlyArray<FlowComponentCatalogItem> = FLOW_COMPONENT_CATALOG,
): FlowComponentCatalogItem | undefined {
  return catalog.find((item) => (
    item.definition.id === componentId
    && item.definition.version === componentVersion
  ));
}
