import { useCallback, useEffect, useRef, useState } from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type {
  JsonObject,
  JsonValue,
  WorkflowInputDefinition,
} from '../model/contracts';
import {
  DEFAULT_RUN_INPUT_VALUES,
  DEFAULT_WORKFLOW_INPUTS,
} from '../model/defaultWorkflowTemplate';
import type { WorkflowEdgeData, WorkflowNodeData } from '../model/workflowModel';
import {
  countWorkflowReferences,
  findExpressionReferenceLocations,
  renameWorkflowReferences,
  type WorkflowReferenceRename,
} from '../values/workflowValueReferences';
import {
  normalizeRunInputValues,
  validateRunInputValues,
} from './workflowRunInputs';

/** 由输入 CRUD API 接收的只读输入声明。 */
export type WorkflowInputDefinitionInput = Readonly<WorkflowInputDefinition>;

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
  const [valuesError, setValuesError] = useState<string | null>(null);
  /** 用于同一批处理内连续结构化操作读取最新的瞬时输入值。 */
  const valuesRef = useRef<JsonObject>(DEFAULT_RUN_INPUT_VALUES);

  /** 提交不进入 Flow 历史的本次运行输入，并同步兼容 JSON 草稿。 */
  const commitRunInputValues = useCallback((nextValues: JsonObject) => {
    valuesRef.current = nextValues;
    setValues(nextValues);
    setValuesError(null);
  }, []);

  /** 将输入声明与运行值保持精确一致，新增文本输入默认使用空字符串。 */
  const syncRunInputValues = useCallback((nextDefinitions: ReadonlyArray<WorkflowInputDefinition>) => {
    const nextValues = normalizeRunInputValues(nextDefinitions, valuesRef.current);
    if (JSON.stringify(nextValues) !== JSON.stringify(valuesRef.current)) {
      commitRunInputValues(nextValues);
    }
  }, [commitRunInputValues]);

  /** 提交结构化输入声明，并保留旧 Inspector 依赖的格式化草稿字段。 */
  const commitDefinitions = useCallback((
    nextDefinitions: WorkflowInputDefinition[],
    draft: string,
    referenceRename?: WorkflowReferenceRename,
  ) => {
    store.getState().transact((document) => ({
      ...document,
      metadata: { ...document.metadata, inputs: nextDefinitions },
      nodes: referenceRename
        ? renameWorkflowReferences(document.nodes, referenceRename)
        : document.nodes,
    }), 'workflow-inputs');
    setDefinitionsDraft(draft);
    setDefinitionsError(null);
    syncRunInputValues(nextDefinitions);
  }, [store, syncRunInputValues]);

  /** 统一把结构化输入操作的失败转换为现有 Inspector 可显示的错误。 */
  const setDefinitionOperationError = useCallback((error: unknown) => {
    setDefinitionsError(error instanceof Error ? error.message : '输入字段操作失败。');
  }, []);

  useEffect(() => {
    try {
      if (JSON.stringify(JSON.parse(definitionsDraft)) !== JSON.stringify(definitions)) {
        setDefinitionsDraft(JSON.stringify(definitions, null, 2));
      }
    } catch {
      // 非法草稿必须保留给用户修正，不能被撤销历史覆盖。
    }
  }, [definitions, definitionsDraft]);

  useEffect(() => {
    // Undo/redo 或外部载入工作流也必须清理多余运行值并补齐新增声明。
    syncRunInputValues(definitions);
  }, [definitions, syncRunInputValues]);

  /** 新增一个工作流输入声明；重复名称或空名称不会写入 Store。 */
  const addInput = useCallback((input: WorkflowInputDefinitionInput): boolean => {
    try {
      const definition = normalizeInputDefinition(input);
      const currentDefinitions = readInputDefinitions(store);
      if (currentDefinitions.some((candidate) => candidate.key === definition.key)) {
        throw new Error('输入字段名称不能为空，且不能重复。');
      }
      const nextDefinitions = [...currentDefinitions, definition];
      commitDefinitions(nextDefinitions, formatJson(nextDefinitions));
      return true;
    } catch (error) {
      setDefinitionOperationError(error);
      return false;
    }
  }, [commitDefinitions, setDefinitionOperationError, store]);

  /** 更新输入声明；双参数形式允许在同一操作中重命名输入。 */
  const updateInput = useCallback((
    currentKeyOrDefinition: string | WorkflowInputDefinitionInput,
    nextDefinition?: WorkflowInputDefinitionInput,
  ): boolean => {
    try {
      const currentKey = typeof currentKeyOrDefinition === 'string'
        ? currentKeyOrDefinition
        : currentKeyOrDefinition.key;
      const input = typeof currentKeyOrDefinition === 'string'
        ? nextDefinition
        : currentKeyOrDefinition;
      if (!input) {
        throw new Error('输入字段必须是 JSON 数组，每项包含 key 和文本类型。');
      }
      const definition = normalizeInputDefinition(input);
      const currentDefinitions = readInputDefinitions(store);
      const index = currentDefinitions.findIndex((candidate) => candidate.key === currentKey);
      if (index < 0) throw new Error('输入字段不存在。');
      if (currentDefinitions.some((candidate, candidateIndex) => (
        candidateIndex !== index && candidate.key === definition.key
      ))) {
        throw new Error('输入字段名称不能为空，且不能重复。');
      }
      if (currentKey !== definition.key) {
        const expressionReferences = findExpressionReferenceLocations(
          store.getState().nodes,
          'workflow_input',
          currentKey,
        );
        if (expressionReferences.length > 0) {
          throw new Error(
            `输入参数 '${currentKey}' 被 ${expressionReferences.length} 个高级表达式引用，请先手动更新表达式。`,
          );
        }
      }
      if (currentKey !== definition.key && hasOwn(valuesRef.current, currentKey)) {
        const nextValues: Record<string, JsonValue> = { ...valuesRef.current };
        nextValues[definition.key] = nextValues[currentKey];
        delete nextValues[currentKey];
        commitRunInputValues(nextValues);
      }
      const nextDefinitions = currentDefinitions.map((candidate, candidateIndex) => (
        candidateIndex === index ? definition : candidate
      ));
      commitDefinitions(
        nextDefinitions,
        formatJson(nextDefinitions),
        currentKey === definition.key ? undefined : {
          kind: 'workflow_input',
          oldName: currentKey,
          newName: definition.key,
        },
      );
      return true;
    } catch (error) {
      setDefinitionOperationError(error);
      return false;
    }
  }, [commitDefinitions, commitRunInputValues, setDefinitionOperationError, store]);

  /** 删除输入声明，并移除对应的本次运行值以避免运行时出现多余字段。 */
  const deleteInput = useCallback((key: string): boolean => {
    try {
      const currentDefinitions = readInputDefinitions(store);
      if (!currentDefinitions.some((candidate) => candidate.key === key)) {
        throw new Error('输入字段不存在。');
      }
      const referenceCount = countWorkflowReferences(
        store.getState().nodes,
        'workflow_input',
        key,
      );
      if (referenceCount > 0) {
        throw new Error(`输入参数 '${key}' 仍被引用 ${referenceCount} 处，请先移除引用。`);
      }
      const nextDefinitions = currentDefinitions.filter((candidate) => candidate.key !== key);
      commitDefinitions(nextDefinitions, formatJson(nextDefinitions));
      return true;
    } catch (error) {
      setDefinitionOperationError(error);
      return false;
    }
  }, [commitDefinitions, setDefinitionOperationError, store]);

  /** 更新单个本次运行值；该状态不进入工作流文档历史。 */
  const setRunInputValue = useCallback((key: string, value: JsonValue): boolean => {
    if (!key.trim()) {
      setValuesError('输入字段名称不能为空。');
      return false;
    }
    if (!readInputDefinitions(store).some((definition) => definition.key === key)) {
      setValuesError('输入字段不存在。');
      return false;
    }
    const nextValues: Record<string, JsonValue> = { ...valuesRef.current, [key]: value };
    commitRunInputValues(nextValues);
    return true;
  }, [commitRunInputValues, store]);

  /** 整组替换本次运行输入，供提交对话框绕过逐字段状态更新时序。 */
  const replaceRunInputValues = useCallback((nextValues: JsonObject): boolean => {
    const validation = validateRunInputValues(readInputDefinitions(store), nextValues);
    if (!validation.valid) {
      setValuesError(validation.message);
      return false;
    }
    commitRunInputValues(validation.values);
    return true;
  }, [commitRunInputValues, store]);

  /** Advanced：从 JSON 导入完整输入声明，并规范化成功后的草稿格式。 */
  const importInputsFromJson = useCallback((draft: string): boolean => {
    setDefinitionsDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      const nextDefinitions = parseInputDefinitions(parsed);
      assertRemovedInputsUnreferenced(store, nextDefinitions);
      commitDefinitions(nextDefinitions, formatJson(nextDefinitions));
      return true;
    } catch (error) {
      setDefinitionsError(error instanceof SyntaxError
        ? 'JSON 格式有误，请检查引号、括号和逗号。'
        : error instanceof Error ? error.message : '输入字段操作失败。');
      return false;
    }
  }, [commitDefinitions, store]);

  /** Advanced：导出 Store 中当前有效的输入声明，而不是导出可能非法的草稿。 */
  const exportInputsAsJson = useCallback(() => (
    formatJson(readInputDefinitions(store))
  ), [store]);

  return {
    inputDefinitions: definitions,
    inputDefinitionsDraft: definitionsDraft,
    inputDefinitionsError: definitionsError,
    addInput,
    updateInput,
    deleteInput,
    importInputsFromJson,
    exportInputsAsJson,
    runInputValues: values,
    runInputValuesError: valuesError,
    setRunInputValue,
    replaceRunInputValues,
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

/** 解析并校验完整输入声明，统一结构化和 JSON API 的运行时边界。 */
function parseInputDefinitions(value: unknown): WorkflowInputDefinition[] {
  if (!isInputDefinitions(value)) {
    throw new Error('输入字段必须是 JSON 数组，每项包含 key 和文本类型。');
  }
  const uniqueKeys = new Set(value.map((definition) => definition.key));
  if (uniqueKeys.size !== value.length || value.some(({ key }) => !key.trim())) {
    throw new Error('输入字段名称不能为空，且不能重复。');
  }
  return value.map(({ key }) => ({ key, value_type: 'text' }));
}

/** 收窄单个结构化输入声明，并拒绝超出当前 schema v8 的输入类型。 */
function normalizeInputDefinition(
  value: WorkflowInputDefinitionInput,
): WorkflowInputDefinition {
  if (!isJsonObject(value) || typeof value.key !== 'string' || value.value_type !== 'text') {
    throw new Error('输入字段必须是 JSON 数组，每项包含 key 和文本类型。');
  }
  if (!value.key.trim()) throw new Error('输入字段名称不能为空，且不能重复。');
  return { key: value.key, value_type: 'text' };
}

/** 读取当前 Store 中的输入声明，避免结构化操作依赖过期闭包。 */
function readInputDefinitions(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
): WorkflowInputDefinition[] {
  const value = store.getState().metadata.inputs;
  return isInputDefinitions(value) ? value : [];
}

/** Advanced 整体导入也必须遵守声明删除保护，不能绕过结构化 CRUD。 */
function assertRemovedInputsUnreferenced(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
  nextDefinitions: ReadonlyArray<WorkflowInputDefinition>,
): void {
  const nextKeys = new Set(nextDefinitions.map((definition) => definition.key));
  const removedKey = readInputDefinitions(store)
    .map((definition) => definition.key)
    .find((key) => !nextKeys.has(key)
      && countWorkflowReferences(store.getState().nodes, 'workflow_input', key) > 0);
  if (removedKey !== undefined) {
    throw new Error(`输入参数 '${removedKey}' 仍被节点引用，请先移除引用。`);
  }
}

/** 统一使用可复制的缩进 JSON 作为高级编辑器和结构化状态的显示格式。 */
function formatJson(value: unknown): string {
  return JSON.stringify(value, null, 2) ?? '';
}

/** 检查 JSON 对象是否包含指定键，兼容可能来自历史快照的原型键名称。 */
function hasOwn(value: JsonObject, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

/** 排除 null 和数组后的 JSON 对象类型守卫。 */
function isJsonObject(value: unknown): value is JsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
