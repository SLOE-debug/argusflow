import {
  createContext,
  useContext,
  useRef,
  type PropsWithChildren,
} from 'react';
import { useStore, type StoreApi } from 'zustand';

import type { FlowState } from './flowStoreTypes';

export { createFlowStore } from './createFlowStore';
export type {
  ConnectionDraft,
  FlowState,
  SelectionBox,
} from './flowStoreTypes';

/** React Context 仅承担 store 实例定位，泛型数据由创建方及业务门面约束。 */
const FlowStoreContext = createContext<StoreApi<FlowState<any, any>> | null>(null);

type FlowProviderProps<TData, TEdgeData> = PropsWithChildren<Readonly<{
  store: StoreApi<FlowState<TData, TEdgeData>>;
}>>;

/** 向自研 Flow 组件树注入独立 Zustand store。 */
export function FlowProvider<TData, TEdgeData>({
  store,
  children,
}: FlowProviderProps<TData, TEdgeData>) {
  const stableStore = useRef(store);
  return (
    <FlowStoreContext.Provider value={stableStore.current}>
      {children}
    </FlowStoreContext.Provider>
  );
}

/** 使用 selector 订阅 Flow store 的最小切片。 */
export function useFlowStore<T>(
  selector: (state: FlowState<any, any>) => T,
): T {
  const store = useContext(FlowStoreContext);
  if (!store) throw new Error('useFlowStore 必须在 FlowProvider 内使用');
  return useStore(store, selector);
}

/** 在非 React 回调中获取当前 Flow store。 */
export function useFlowStoreApi(): StoreApi<FlowState<any, any>> {
  const store = useContext(FlowStoreContext);
  if (!store) throw new Error('useFlowStoreApi 必须在 FlowProvider 内使用');
  return store;
}
