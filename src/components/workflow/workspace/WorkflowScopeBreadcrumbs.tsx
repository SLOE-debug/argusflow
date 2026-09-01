import ChevronRight from 'lucide-react/dist/esm/icons/chevron-right.mjs';
import type { StoreApi } from 'zustand';
import { useStore } from 'zustand';

import type { FlowState } from '../../../flow';
import type {
  WorkflowEdgeData,
  WorkflowNodeData,
  WorkflowScopeMetadataMap,
} from '../../../features/workflow';

type WorkflowScopeBreadcrumbsProps = Readonly<{
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>;
  onOpenScope: (scopeId: string) => boolean;
}>;

/** 展示当前 While 嵌套路径，并允许一次跳回任意祖先。 */
export function WorkflowScopeBreadcrumbs({
  store,
  onOpenScope,
}: WorkflowScopeBreadcrumbsProps) {
  const activeScopeId = useStore(store, (state) => state.activeDocumentId);
  const rootScopeId = useStore(store, (state) => state.metadata.rootScopeId as string);
  const scopeMetadata = useStore(
    store,
    (state) => state.metadata.scopeMetadata as WorkflowScopeMetadataMap,
  );
  const path = resolveScopePath(activeScopeId, rootScopeId, scopeMetadata);
  if (path.length <= 1) return null;
  return (
    <nav
      aria-label="流程作用域"
      className="pointer-events-auto absolute top-3 left-1/2 z-40 flex -translate-x-1/2 items-center rounded-lg border border-slate-200 bg-white/95 px-2 py-1 shadow-lg backdrop-blur"
    >
      {path.map((scopeId, index) => (
        <div key={scopeId} className="flex items-center">
          {index > 0 ? <ChevronRight className="mx-1 size-3 text-slate-300" aria-hidden="true" /> : null}
          <button
            type="button"
            className={scopeId === activeScopeId
              ? 'rounded px-2 py-1 text-[11px] font-semibold text-violet-700'
              : 'rounded px-2 py-1 text-[11px] text-slate-500 hover:bg-slate-100 hover:text-slate-800'}
            onClick={() => onOpenScope(scopeId)}
          >
            {scopeId === rootScopeId ? '主流程' : resolveScopeLabel(scopeId, scopeMetadata, store)}
          </button>
        </div>
      ))}
    </nav>
  );
}

/** 迭代构造根到当前作用域路径，允许任意 While 嵌套深度。 */
function resolveScopePath(
  activeScopeId: string,
  rootScopeId: string,
  metadata: WorkflowScopeMetadataMap,
): string[] {
  const path = [activeScopeId];
  const visited = new Set(path);
  let current = activeScopeId;
  while (current !== rootScopeId) {
    const parent = metadata[current]?.parent?.scope_id;
    if (!parent || visited.has(parent)) break;
    path.push(parent);
    visited.add(parent);
    current = parent;
  }
  return path.reverse();
}

/** 使用父容器节点标题作为比作用域 ID 更友好的面包屑。 */
function resolveScopeLabel(
  scopeId: string,
  metadata: WorkflowScopeMetadataMap,
  store: StoreApi<FlowState<WorkflowNodeData, WorkflowEdgeData>>,
): string {
  const parent = metadata[scopeId]?.parent;
  if (!parent) return scopeId;
  const owner = store.getState().documents[parent.scope_id]?.nodes
    .find((node) => node.id === parent.node_id);
  return owner?.data.label ?? 'While';
}
