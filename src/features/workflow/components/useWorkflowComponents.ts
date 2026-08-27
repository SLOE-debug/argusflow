import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import type { StoreApi } from 'zustand';

import type { FlowState } from '../../../flow';
import type { ValidationReport } from '../model/contracts';
import {
  FLOW_COMPONENT_CATALOG,
  type FlowComponentCatalogItem,
} from './componentCatalog';
import {
  ComponentCreationError,
  createComponentFromSelection,
} from './componentCreation';
import type { WorkflowEdgeData, WorkflowNodeData } from '../model/workflowModel';

type WorkflowFlowStore = StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;

/** 管理内置与工作区组件目录，并封装“从选择创建组件”的文档事务。 */
export function useWorkflowComponents(
  flowStore: WorkflowFlowStore,
  setErrorMessage: Dispatch<SetStateAction<string | null>>,
  setValidationReport: Dispatch<SetStateAction<ValidationReport | null>>,
) {
  /** 内置组件与本工作区从选择创建的组件目录。 */
  const [componentCatalog, setComponentCatalog] = useState<
    ReadonlyArray<FlowComponentCatalogItem>
  >(FLOW_COMPONENT_CATALOG);

  /** 把当前连续选择原地折叠成一个工作区组件。 */
  const createComponent = useCallback((name: string, version: string) => {
    const state = flowStore.getState();
    try {
      const result = createComponentFromSelection(
        state.nodes,
        state.edges,
        state.selectedNodeIds,
        name,
        version,
      );
      state.transact((document) => ({
        ...document,
        nodes: result.nodes,
        edges: result.edges,
      }), 'create-component');
      flowStore.getState().selectNodes([result.componentNodeId]);
      setComponentCatalog((current) => [...current, result.catalogItem]);
      setErrorMessage(null);
      setValidationReport(null);
      return true;
    } catch (error) {
      setErrorMessage(error instanceof ComponentCreationError
        ? error.message
        : '创建流程组件失败');
      return false;
    }
  }, [flowStore, setErrorMessage, setValidationReport]);

  return { componentCatalog, createComponent } as const;
}
