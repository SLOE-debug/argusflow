import { useEffect, useMemo, useState } from 'react';

import { useFlowStoreApi } from '../store/store';
import type { FlowPoint, NodeRegistry } from '../types';

/** 连续方向键微调共用历史分组，一次长按只需一次撤销。 */
const KEYBOARD_NUDGE_HISTORY_GROUP = 'flow.keyboard-nudge';

/** 单次方向键微调的画布逻辑像素。 */
const NUDGE_DISTANCE = 1;

/** 按住 Shift 时单次方向键移动的画布逻辑像素。 */
const FAST_NUDGE_DISTANCE = 10;

/** 方向键到单位位移的强类型映射。 */
const NUDGE_DIRECTIONS = {
  ArrowUp: { x: 0, y: -1 },
  ArrowRight: { x: 1, y: 0 },
  ArrowDown: { x: 0, y: 1 },
  ArrowLeft: { x: -1, y: 0 },
} as const satisfies Readonly<Record<string, FlowPoint>>;

type NudgeKey = keyof typeof NUDGE_DIRECTIONS;

/** 判断键盘事件是否来自需要保留原生文本编辑行为的元素。 */
function isEditable(target: EventTarget | null): boolean {
  return target instanceof HTMLElement
    && (target.matches('input, textarea, select') || target.isContentEditable);
}

/** 判断目标是否需要自行处理方向键导航。 */
function ownsArrowKeyNavigation(target: EventTarget | null): boolean {
  return target instanceof HTMLElement
    && (
      target.matches('button, a[href]')
      || target.closest('[role="dialog"], [role="menu"]') !== null
    );
}

/** 将可支持的方向键收窄为微调键。 */
function isNudgeKey(key: string): key is NudgeKey {
  return key in NUDGE_DIRECTIONS;
}

/** 从方向键和 Shift 加速状态计算节点位移。 */
function resolveNudgeDelta(event: KeyboardEvent): FlowPoint | null {
  if (
    event.ctrlKey
    || event.metaKey
    || event.altKey
    || !isNudgeKey(event.key)
  ) return null;

  const distance = event.shiftKey ? FAST_NUDGE_DISTANCE : NUDGE_DISTANCE;
  const direction = NUDGE_DIRECTIONS[event.key];
  return {
    x: direction.x * distance,
    y: direction.y * distance,
  };
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

      const nudgeDelta = resolveNudgeDelta(event);
      if (
        nudgeDelta
        && !ownsArrowKeyNavigation(event.target)
        && state.selectedNodeIds.size > 0
      ) {
        event.preventDefault();
        state.moveSelected(
          nudgeDelta,
          true,
          KEYBOARD_NUDGE_HISTORY_GROUP,
        );
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
