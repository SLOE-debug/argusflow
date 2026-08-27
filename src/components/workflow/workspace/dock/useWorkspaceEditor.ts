import { useCallback, useState } from 'react';

import type {
  StructuredEditorTarget,
  WorkspaceEditorMode,
  WorkspaceEditorState,
} from './structuredEditorTarget';

/** Dock 在常见桌面窗口中的默认高度边界。 */
export const WORKSPACE_DOCK_HEIGHT = {
  min: 220,
  preferredMin: 320,
  preferredMax: 520,
} as const;

/** 根据当前视口计算 `clamp(320px, 38vh, 520px)` 的初始高度。 */
function createInitialDockHeight(): number {
  const viewportHeight = typeof window === 'undefined' ? 900 : window.innerHeight;
  return Math.min(
    WORKSPACE_DOCK_HEIGHT.preferredMax,
    Math.max(WORKSPACE_DOCK_HEIGHT.preferredMin, viewportHeight * 0.38),
  );
}

/** 管理不参与工作流持久化的结构化编辑器目标与 Dock 布局。 */
export function useWorkspaceEditor() {
  const [state, setState] = useState<WorkspaceEditorState>(() => ({
    target: null,
    mode: 'docked',
    dockHeight: createInitialDockHeight(),
  }));

  /** 打开目标文档时保留用户上次调整的 Dock 高度。 */
  const openEditor = useCallback((target: StructuredEditorTarget) => {
    setState((current) => ({ ...current, target }));
  }, []);

  /** 关闭文档后回到普通工具 Dock，避免把日志区留在最大化状态。 */
  const closeEditor = useCallback(() => {
    setState((current) => ({ ...current, target: null, mode: 'docked' }));
  }, []);

  /** 在 Dock 与中央工作区最大化之间切换，不改变当前文档。 */
  const setMode = useCallback((mode: WorkspaceEditorMode) => {
    setState((current) => ({ ...current, mode }));
  }, []);

  /** 保存经过 Workspace 布局约束的用户高度。 */
  const setDockHeight = useCallback((dockHeight: number) => {
    setState((current) => ({ ...current, dockHeight }));
  }, []);

  return {
    state,
    openEditor,
    closeEditor,
    setMode,
    setDockHeight,
  } as const;
}
