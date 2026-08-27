import type {
  UiExecutionPolicy,
  UiOperation,
} from '../model/contracts';
import {
  createBackendPolicy,
  createDefaultUiExecutionPolicy,
  createDefaultUiOperation,
} from './workflowAction';

/** Node Preset 只预填一个 Primitive，不引入新的 Runtime type。 */
export type NodePresetDefinition = Readonly<{
  id: string;
  title: string;
  description: string;
  label: string;
  operation: () => UiOperation;
  execution: () => UiExecutionPolicy;
}>;

/** Studio 内置的单节点易用性预设。 */
export const NODE_PRESET_CATALOG = [
  defineUiPreset('click', '点击', '点击界面上的元素', '点击', () => (
    createDefaultUiOperation()
  )),
  defineUiPreset('set-value', '输入文字', '在界面上输入文字', '输入文字', () => ({
    ...createDefaultUiOperation(),
    type: 'set_value',
    value: { type: 'literal', value: '' },
  })),
  defineUiPreset('get-text', '读取文字', '读取界面上的文字', '读取文字', () => ({
    ...createDefaultUiOperation(),
    type: 'get_text',
  })),
  defineUiPreset('get-value', '读取控件值', '读取控件里的值', '读取控件值', () => ({
    ...createDefaultUiOperation(),
    type: 'get_value',
  })),
  defineUiPreset('extract-links', '读取网页链接', '批量读取网页上的链接', '读取网页链接', () => ({
    type: 'extract',
    target: {
      ...createDefaultUiOperation().target,
      locator: {
        type: 'query',
        query: { language_version: 1, source: 'css("a[href]")' },
      },
      backend_policy: createBackendPolicy('browser_cdp'),
    },
    cardinality: 'many',
    fields: [
      { name: 'title', source: { type: 'text' } },
      { name: 'url', source: { type: 'attribute', name: 'href' } },
    ],
  })),
] as const satisfies ReadonlyArray<NodePresetDefinition>;

/** 通过稳定 ID 查找节点预设。 */
export function findNodePreset(id: string): NodePresetDefinition | undefined {
  return NODE_PRESET_CATALOG.find((preset) => preset.id === id);
}

/** 装配一个沿用统一目标等待策略的 UI 预设。 */
function defineUiPreset(
  id: string,
  title: string,
  description: string,
  label: string,
  operation: () => UiOperation,
): NodePresetDefinition {
  return {
    id,
    title,
    description,
    label,
    operation,
    execution: createDefaultUiExecutionPolicy,
  };
}
