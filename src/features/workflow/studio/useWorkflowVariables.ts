import { useCallback, useEffect, useState } from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type { JsonObject, JsonValue } from '../model/contracts';
import { DEFAULT_WORKFLOW_VARIABLES } from '../model/defaultWorkflowTemplate';
import type { WorkflowEdgeData, WorkflowNodeData } from '../model/workflowModel';
import {
  countWorkflowReferences,
  findExpressionReferenceLocations,
  renameWorkflowReferences,
  type WorkflowReferenceRename,
} from '../values/workflowValueReferences';

/** 管理工作流级初始变量及高级 JSON 导入草稿的结构化 Hook。 */
export function useWorkflowVariables(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
) {
  const variables = useStore(
    store,
    (state) => state.metadata.variables as JsonObject,
  );
  const [variablesDraft, setVariablesDraft] = useState(
    JSON.stringify(DEFAULT_WORKFLOW_VARIABLES, null, 2),
  );
  const [variablesError, setVariablesError] = useState<string | null>(null);

  /** 提交工作流变量并同步旧 Inspector 仍使用的 JSON 草稿。 */
  const commitVariables = useCallback((
    nextVariables: JsonObject,
    draft: string,
    referenceRename?: WorkflowReferenceRename,
  ) => {
    store.getState().transact((document) => ({
      ...document,
      metadata: { ...document.metadata, variables: nextVariables },
      nodes: referenceRename
        ? renameWorkflowReferences(document.nodes, referenceRename)
        : document.nodes,
    }), 'workflow-variables');
    setVariablesDraft(draft);
    setVariablesError(null);
  }, [store]);

  useEffect(() => {
    try {
      if (JSON.stringify(JSON.parse(variablesDraft)) !== JSON.stringify(variables)) {
        setVariablesDraft(JSON.stringify(variables, null, 2));
      }
    } catch {
      // 非法草稿必须保留给用户修正，不能被历史状态覆盖。
    }
  }, [variables, variablesDraft]);

  /** 新建或更新一个工作流变量。 */
  const setVariable = useCallback((name: string, value: JsonValue): boolean => {
    if (!name.trim()) {
      setVariablesError('变量名称不能为空。');
      return false;
    }
    const currentVariables = readVariables(store);
    commitVariables(
      { ...currentVariables, [name]: value },
      formatJson({ ...currentVariables, [name]: value }),
    );
    return true;
  }, [commitVariables, store]);

  /** 新建变量；与更新接口分开，避免误覆盖同名声明。 */
  const addVariable = useCallback((name: string, value: JsonValue): boolean => {
    if (!name.trim()) {
      setVariablesError('变量名称不能为空。');
      return false;
    }
    const currentVariables = readVariables(store);
    if (hasOwn(currentVariables, name)) {
      setVariablesError('变量名称已存在。');
      return false;
    }
    const nextVariables = { ...currentVariables, [name]: value };
    commitVariables(nextVariables, formatJson(nextVariables));
    return true;
  }, [commitVariables, store]);

  /** 原子更新变量名称和初始值，避免重命名过程中短暂丢失旧值。 */
  const updateVariable = useCallback((
    oldName: string,
    newName: string,
    value: JsonValue,
  ): boolean => {
    if (!newName.trim()) {
      setVariablesError('变量名称不能为空。');
      return false;
    }
    const currentVariables = readVariables(store);
    if (!hasOwn(currentVariables, oldName)) {
      setVariablesError('变量不存在。');
      return false;
    }
    if (oldName !== newName && hasOwn(currentVariables, newName)) {
      setVariablesError('变量名称已存在。');
      return false;
    }
    if (oldName !== newName && hasExpressionReferences(store, oldName)) {
      setVariablesError(`变量 '${oldName}' 被高级表达式引用，请先手动更新表达式。`);
      return false;
    }
    const nextVariables: Record<string, JsonValue> = { ...currentVariables };
    if (oldName !== newName) delete nextVariables[oldName];
    nextVariables[newName] = value;
    commitVariables(
      nextVariables,
      formatJson(nextVariables),
      oldName === newName ? undefined : {
        kind: 'variable',
        oldName,
        newName,
      },
    );
    return true;
  }, [commitVariables, store]);

  /** 删除一个已声明的工作流变量。 */
  const deleteVariable = useCallback((name: string): boolean => {
    const currentVariables = readVariables(store);
    if (!hasOwn(currentVariables, name)) {
      setVariablesError('变量不存在。');
      return false;
    }
    const referenceCount = countWorkflowReferences(
      store.getState().nodes,
      'variable',
      name,
    );
    if (referenceCount > 0) {
      setVariablesError(`变量 '${name}' 仍被引用 ${referenceCount} 处，请先移除引用。`);
      return false;
    }
    const nextVariables: Record<string, JsonValue> = { ...currentVariables };
    delete nextVariables[name];
    commitVariables(nextVariables, formatJson(nextVariables));
    return true;
  }, [commitVariables, store]);

  /** 重命名变量并在同一历史事务内同步结构化引用。 */
  const renameVariable = useCallback((oldName: string, newName: string): boolean => {
    if (!oldName.trim() || !newName.trim()) {
      setVariablesError('变量名称不能为空。');
      return false;
    }
    const currentVariables = readVariables(store);
    if (!hasOwn(currentVariables, oldName)) {
      setVariablesError('变量不存在。');
      return false;
    }
    if (oldName !== newName && hasOwn(currentVariables, newName)) {
      setVariablesError('变量名称已存在。');
      return false;
    }
    if (oldName === newName) return true;
    if (hasExpressionReferences(store, oldName)) {
      setVariablesError(`变量 '${oldName}' 被高级表达式引用，请先手动更新表达式。`);
      return false;
    }
    const nextVariables: Record<string, JsonValue> = { ...currentVariables };
    nextVariables[newName] = nextVariables[oldName];
    delete nextVariables[oldName];
    commitVariables(nextVariables, formatJson(nextVariables), {
      kind: 'variable',
      oldName,
      newName,
    });
    return true;
  }, [commitVariables, store]);

  /** Advanced：以 JSON 对象整体替换工作流变量。 */
  const replaceVariablesFromJson = useCallback((draft: string): boolean => {
    setVariablesDraft(draft);
    try {
      const parsed: unknown = JSON.parse(draft);
      const nextVariables = parseVariablesObject(parsed);
      assertRemovedVariablesUnreferenced(store, nextVariables);
      commitVariables(nextVariables, formatJson(nextVariables));
      return true;
    } catch (error) {
      setVariablesError(error instanceof SyntaxError
        ? 'JSON 格式有误，请检查引号、括号和逗号。'
        : error instanceof Error ? error.message : '工作流变量操作失败。');
      return false;
    }
  }, [commitVariables, store]);

  return {
    variables,
    variablesDraft,
    variablesError,
    setVariable,
    addVariable,
    updateVariable,
    deleteVariable,
    renameVariable,
    replaceVariablesFromJson,
  };
}

/** 读取当前 Store 变量，避免连续结构化操作使用过期 Hook 闭包。 */
function readVariables(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
): JsonObject {
  const value: unknown = store.getState().metadata.variables;
  return isJsonObject(value) ? value : {};
}

/** 解析变量 JSON，并建立根值必须是对象的领域边界。 */
function parseVariablesObject(value: unknown): JsonObject {
  if (!isJsonObject(value)) throw new Error('变量必须是 JSON 对象。');
  return value;
}

/** 格式化高级 JSON 编辑器使用的稳定缩进文本。 */
function formatJson(value: JsonValue): string {
  return JSON.stringify(value, null, 2) ?? '';
}

/** 判断对象是否自有指定变量键。 */
function hasOwn(value: JsonObject, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

/** 排除 null 和数组后的 JSON 对象类型守卫。 */
function isJsonObject(value: unknown): value is JsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

/** 高级表达式是源码字符串，命中时必须拒绝不安全的自动重写。 */
function hasExpressionReferences(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
  name: string,
): boolean {
  return findExpressionReferenceLocations(store.getState().nodes, 'variable', name).length > 0;
}

/** Advanced 整体导入不得绕过变量删除保护，否则会留下不可发现的失效引用。 */
function assertRemovedVariablesUnreferenced(
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
  nextVariables: JsonObject,
): void {
  const removedName = Object.keys(readVariables(store)).find((name) => (
    !hasOwn(nextVariables, name)
    && countWorkflowReferences(store.getState().nodes, 'variable', name) > 0
  ));
  if (removedName !== undefined) {
    throw new Error(`变量 '${removedName}' 仍被节点引用，请先移除引用。`);
  }
}
