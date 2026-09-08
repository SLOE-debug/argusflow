import { useLayoutEffect, useState, type RefObject } from 'react';

/** 根据视口可用空间翻转菜单，固定定位不参与父容器滚动布局。 */
export function useSelectPlacement(open: boolean, trigger: RefObject<HTMLButtonElement | null>, menu: RefObject<HTMLDivElement | null>) {
  const [placement, setPlacement] = useState({ left: 0, top: 0, width: 0, maxHeight: 256 });
  useLayoutEffect(() => {
    if (!open) return;
    const update = () => {
      if (!trigger.current || !menu.current) return;
      const bounds = trigger.current.getBoundingClientRect();
      const viewport = window.visualViewport;
      const height = viewport?.height ?? window.innerHeight;
      const width = viewport?.width ?? window.innerWidth;
      const below = Math.max(0, height - bounds.bottom - 14);
      const above = Math.max(0, bounds.top - 14);
      const desired = Math.min(256, menu.current.scrollHeight);
      const upward = below < desired && above > below;
      const maxHeight = Math.min(256, upward ? above : below);
      const menuWidth = Math.min(bounds.width, width - 16);
      setPlacement({ left: Math.max(8, Math.min(bounds.left, width - menuWidth - 8)), top: upward ? bounds.top - Math.min(desired, maxHeight) - 6 : bounds.bottom + 6, width: menuWidth, maxHeight });
    };
    update();
    const observer = new ResizeObserver(update);
    if (trigger.current) observer.observe(trigger.current);
    window.addEventListener('resize', update);
    window.addEventListener('scroll', update, true);
    return () => { observer.disconnect(); window.removeEventListener('resize', update); window.removeEventListener('scroll', update, true); };
  }, [open, trigger, menu]);
  return placement;
}
