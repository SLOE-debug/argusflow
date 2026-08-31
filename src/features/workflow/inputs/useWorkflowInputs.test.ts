import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { createFlowStore } from '../../../flow';
import type { WorkflowEdgeData, WorkflowInputDefinition, WorkflowNodeData } from '../index';
import { useWorkflowInputs } from './useWorkflowInputs';

/** 创建只包含输入元数据的测试工作流，避免把画布节点带入 Hook 单元测试。 */
function createInputStore() {
  const inputs: ReadonlyArray<WorkflowInputDefinition> = [
    { key: 'recipient', value_type: 'text' },
    { key: 'content', value_type: 'text' },
  ];
  return createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
    metadata: { inputs },
    nodes: [],
    edges: [],
  });
}

describe('useWorkflowInputs', () => {
  it('provides structured CRUD and keeps run values synchronized', () => {
    const store = createInputStore();
    const inputs = renderHook(() => useWorkflowInputs(store));

    act(() => {
      expect(inputs.result.current.setRunInputValue('recipient', 'Alice')).toBe(true);
      expect(inputs.result.current.addInput({
        key: 'channel',
        value_type: 'text',
      })).toBe(true);
    });

    expect(store.getState().metadata.inputs).toEqual([
      { key: 'recipient', value_type: 'text' },
      { key: 'content', value_type: 'text' },
      { key: 'channel', value_type: 'text' },
    ]);
    expect(inputs.result.current.runInputValues).toMatchObject({
      recipient: 'Alice',
      channel: '',
    });

    act(() => {
      expect(inputs.result.current.updateInput('recipient', {
        key: 'customer_name',
        value_type: 'text',
      })).toBe(true);
    });
    expect(inputs.result.current.inputDefinitions[0]).toEqual({
      key: 'customer_name',
      value_type: 'text',
    });
    expect(inputs.result.current.runInputValues).toMatchObject({
      customer_name: 'Alice',
    });
    expect(inputs.result.current.runInputValues.recipient).toBeUndefined();

    act(() => {
      expect(inputs.result.current.deleteInput('content')).toBe(true);
    });
    expect(inputs.result.current.inputDefinitions).toHaveLength(2);
    expect(inputs.result.current.runInputValues.content).toBeUndefined();
  });

  it('rejects duplicate definitions and supports advanced JSON import/export', () => {
    const store = createInputStore();
    const inputs = renderHook(() => useWorkflowInputs(store));

    act(() => {
      expect(inputs.result.current.addInput({
        key: 'recipient',
        value_type: 'text',
      })).toBe(false);
    });
    expect(inputs.result.current.inputDefinitionsError).toBe('输入字段名称不能为空，且不能重复。');

    const imported = [{ key: 'email', value_type: 'text' as const }];
    act(() => {
      expect(inputs.result.current.importInputsFromJson(JSON.stringify(imported))).toBe(true);
    });
    expect(inputs.result.current.exportInputsAsJson()).toBe(JSON.stringify(imported, null, 2));
    expect(store.getState().metadata.inputs).toEqual(imported);
  });

  it('can replace the complete run input object in one operation', () => {
    const inputs = renderHook(() => useWorkflowInputs(createInputStore()));
    const nextValues = { recipient: 'Bob', content: 'Hello' } as const;

    act(() => {
      expect(inputs.result.current.replaceRunInputValues(nextValues)).toBe(true);
    });

    expect(inputs.result.current.runInputValues).toEqual(nextValues);
  });

  it('rejects missing, unexpected and non-text run input values', () => {
    const inputs = renderHook(() => useWorkflowInputs(createInputStore()));

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({ recipient: 'Bob' })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("缺少输入参数 'content'。");

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({
        recipient: 'Bob',
        content: 'Hello',
        extra: 'no',
      })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("输入参数 'extra' 未声明。");

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({
        recipient: 1,
        content: 'Hello',
      })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("输入参数 'recipient' 必须是文本。");
  });
});
