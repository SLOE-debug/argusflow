import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { createFlowStore } from '../../../flow';
import {
  DEFAULT_WORKFLOW_INPUTS,
  type WorkflowEdgeData,
  type WorkflowNodeData,
} from '../index';
import { useWorkflowInputs } from './useWorkflowInputs';

/** 创建只包含输入元数据的测试工作流，避免把画布节点带入 Hook 单元测试。 */
function createInputStore() {
  return createFlowStore<WorkflowNodeData, WorkflowEdgeData>({
    metadata: { inputs: DEFAULT_WORKFLOW_INPUTS },
    nodes: [],
    edges: [],
  });
}

describe('useWorkflowInputs', () => {
  it('provides structured CRUD and keeps run values synchronized', () => {
    const store = createInputStore();
    const inputs = renderHook(() => useWorkflowInputs(store));

    act(() => {
      expect(inputs.result.current.setRunInputValue('contact_name', 'Alice')).toBe(true);
      expect(inputs.result.current.addInput({
        key: 'channel',
        value_type: 'text',
      })).toBe(true);
    });

    expect(store.getState().metadata.inputs).toEqual([
      { key: 'contact_name', value_type: 'text' },
      { key: 'message', value_type: 'text' },
      { key: 'channel', value_type: 'text' },
    ]);
    expect(inputs.result.current.runInputValues).toMatchObject({
      contact_name: 'Alice',
      channel: '',
    });

    act(() => {
      expect(inputs.result.current.updateInput('contact_name', {
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
    expect(inputs.result.current.runInputValues.contact_name).toBeUndefined();

    act(() => {
      expect(inputs.result.current.deleteInput('message')).toBe(true);
    });
    expect(inputs.result.current.inputDefinitions).toHaveLength(2);
    expect(inputs.result.current.runInputValues.message).toBeUndefined();
  });

  it('rejects duplicate definitions and supports advanced JSON import/export', () => {
    const store = createInputStore();
    const inputs = renderHook(() => useWorkflowInputs(store));

    act(() => {
      expect(inputs.result.current.addInput({
        key: 'contact_name',
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
    const nextValues = { contact_name: 'Bob', message: 'Hello' } as const;

    act(() => {
      expect(inputs.result.current.replaceRunInputValues(nextValues)).toBe(true);
    });

    expect(inputs.result.current.runInputValues).toEqual(nextValues);
  });

  it('rejects missing, unexpected and non-text run input values', () => {
    const inputs = renderHook(() => useWorkflowInputs(createInputStore()));

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({ contact_name: 'Bob' })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("缺少输入参数 'message'。");

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({
        contact_name: 'Bob',
        message: 'Hello',
        extra: 'no',
      })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("输入参数 'extra' 未声明。");

    act(() => {
      expect(inputs.result.current.replaceRunInputValues({
        contact_name: 1,
        message: 'Hello',
      })).toBe(false);
    });
    expect(inputs.result.current.runInputValuesError).toBe("输入参数 'contact_name' 必须是文本。");
  });
});
