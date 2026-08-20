/** 节点选择的三种合并方式。 */
export type NodeSelectionMode = 'replace' | 'add' | 'toggle';

/** 根据选择模式返回新的节点 ID 集合，不修改现有 Set。 */
export function updateNodeSelection(
  current: ReadonlySet<string>,
  ids: Iterable<string>,
  mode: NodeSelectionMode,
): Set<string> {
  const nextSelection = mode === 'replace'
    ? new Set<string>()
    : new Set(current);

  for (const id of ids) {
    if (mode === 'toggle' && nextSelection.has(id)) nextSelection.delete(id);
    else nextSelection.add(id);
  }

  return nextSelection;
}
