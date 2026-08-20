import { useEffect, useMemo, useState } from 'react';

import { useFlowStoreApi } from './store';
import type { NodeRegistry } from './types';

/** 判断键盘事件是否来自需要保留原生文本编辑行为的元素。 */
function isEditable(target: EventTarget | null): boolean {
  return target instanceof HTMLElement
    && (target.matches('input, textarea, select') || target.isContentEditable);
}

/** 从注册表计算粘贴和复制时不可重复创建的节点类型。 */
function collectSingletonKinds(registry: Readonly<NodeRegistry>): ReadonlySet<string> {
  return new Set(
    Object.values(registry)
      .filter((definition) => definition.singleton)
      .map((definition) => definition.kind),
  );
}

/** 注册画布级快捷键，并返回空格键平移模式是否已开启。 */
export function useCanvasKeyboard(registry: Readonly<NodeRegistry>): boolean {
  const store = useFlowStoreApi();
  const [spacePressed, setSpacePressed] = useState(false);
  const singletonKinds = useMemo(() => collectSingletonKinds(registry), [registry]);

  useEffect(() => {
    const handleKeyEvent = (event: KeyboardEvent, pressed: boolean) => {
      if (isEditable(event.target)) return;

      if (event.code === 'Space') {
        event.preventDefault();
        setSpacePressed(pressed);
      }

      if (!pressed) return;

      const state = store.getState();
      const modifierPressed = event.ctrlKey || event.metaKey;
      const normalizedKey = event.key.toLowerCase();

      if (modifierPressed && normalizedKey === 'z') {
        event.preventDefault();
        if (event.shiftKey) state.redo();
        else state.undo();
        return;
      }

      if (modifierPressed && normalizedKey === 'y') {
        event.preventDefault();
        state.redo();
        return;
      }

      if (modifierPressed && normalizedKey === 'c') {
        event.preventDefault();
        state.copy();
        return;
      }

      if (modifierPressed && normalizedKey === 'v') {
        event.preventDefault();
        state.paste(new Set(singletonKinds));
        return;
      }

      if (modifierPressed && normalizedKey === 'd') {
        event.preventDefault();
        state.duplicate(new Set(singletonKinds));
        return;
      }

      if (modifierPressed && normalizedKey === 'a') {
        event.preventDefault();
        state.selectNodes(state.nodes.map((node) => node.id));
        return;
      }

      if (event.key === 'Delete' || event.key === 'Backspace') {
        event.preventDefault();
        state.deleteSelection();
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => handleKeyEvent(event, true);
    const handleKeyUp = (event: KeyboardEvent) => handleKeyEvent(event, false);

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [singletonKinds, store]);

  return spacePressed;
}
