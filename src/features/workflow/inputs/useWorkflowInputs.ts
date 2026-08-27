import { useCallback, useEffect, useState } from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type {
  JsonObject,
  WorkflowInputDefinition,
} from '../model/contracts';
import {
  DEFAULT_RUN_INPUT_VALUES,
  DEFAULT_WORKFLOW_INPUTS,
} from '../model/defaultWorkflowTemplate';
import type { WorkflowEdgeData, WorkflowNodeData } from '../model/workflowModel';

/** 管理持久化输入声明和不进入画布历史的本次运行输入。 */
export function useWorkflowInputs(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
) {
  const definitions = useStore(
    store,
    (state) => state.metadata.inputs as WorkflowInputDefinition[],
  );
  const [definitionsDraft, setDefinitionsDraft] = useState(
    JSON.stringify(DEFAULT_WORKFLOW_INPUTS, null, 2),
  );
  const [definitionsError, setDefinitionsError] = useState<string | null>(null);
  const [values, setValues] = useState<JsonObject>(DEFAULT_RUN_INPUT_VALUES);
  const [valuesDraft, setValuesDraft] = useState(
    JSON.stringify(DEFAULT_RUN_INPUT_VALUES, null, 2),
  );
  const [valuesError, setValuesError] = useState<string | null>(null);

  useEffect(() => {
    try {
      if (JSON.stringify(JSON.parse(definitionsDraft)) !== JSON.stringify(definitions)) {
        setDefinitionsDraft(JSON.stringify(definitions, null, 2));
      }
    } catch {
      // 非法草稿必须保留给用户修正，不能被撤销历史覆盖。
    }
  }, [definitions, definitionsDraft]);

  const updateDefinitions = useCallback((draft: string) => {
    setDefinitionsDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      if (!isInputDefinitions(parsed)) {
        throw new Error('输入字段必须是 JSON 数组，每项包含 key 和文本类型。');
      }
      const uniqueKeys = new Set(parsed.map((definition) => definition.key));
      if (uniqueKeys.size !== parsed.length || parsed.some(({ key }) => !key.trim())) {
        throw new Error('输入字段名称不能为空，且不能重复。');
      }
      store.getState().setMetadata(
        { inputs: parsed },
        true,
        'workflow-inputs',
      );
      setDefinitionsError(null);
    } catch (error) {
      setDefinitionsError(error instanceof Error && error.message.startsWith('输入字段')
        ? error.message
        : 'JSON 格式有误，请检查引号、括号和逗号。');
    }
  }, [store]);

  const updateValues = useCallback((draft: string) => {
    setValuesDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      if (!isJsonObject(parsed)) {
        throw new Error('本次运行输入必须是 JSON 对象');
      }
      setValues(parsed);
      setValuesError(null);
    } catch (error) {
      setValuesError(error instanceof Error && error.message === '本次运行输入必须是 JSON 对象'
        ? error.message
        : 'JSON 格式有误，请检查引号、括号和逗号。');
    }
  }, []);

  return {
    inputDefinitions: definitions,
    inputDefinitionsDraft: definitionsDraft,
    inputDefinitionsError: definitionsError,
    updateInputDefinitions: updateDefinitions,
    runInputValues: values,
    runInputValuesDraft: valuesDraft,
    runInputValuesError: valuesError,
    updateRunInputValues: updateValues,
  };
}

/** 验证未知 JSON 是否符合当前唯一支持的文本输入声明。 */
function isInputDefinitions(value: unknown): value is WorkflowInputDefinition[] {
  return Array.isArray(value) && value.every((definition) => (
    isJsonObject(definition)
    && typeof definition.key === 'string'
    && definition.value_type === 'text'
    && Object.keys(definition).length === 2
  ));
}

/** 排除 null 和数组后的 JSON 对象类型守卫。 */
function isJsonObject(value: unknown): value is JsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
