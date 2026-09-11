import { useEffect, type RefObject } from "react";
import {
  compose,
  contains,
  inverse,
  screenToWorld,
  zoomAt,
} from "../../../flow";
import { studio, type Scene } from "../../../features/workflow";
/** 滚轮只改变仿射相机，作用域切换保留相同屏幕投影。 */
export function useCanvasNavigation(
  host: RefObject<HTMLDivElement | null>,
  scene: Scene,
) {
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      const tab = studio.active;
      if (!tab) return;
      const active = scene.scopes[tab.scope];
      if (!active) return;
      const rect = element.getBoundingClientRect();
      const point = {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      };
      const nextZoom = Math.max(
        0.03,
        Math.min(4, tab.viewport.zoom * Math.exp(-event.deltaY * 0.0015)),
      );
      const next = zoomAt(tab.viewport, point, nextZoom);
      const world = screenToWorld(point, next);
      if (event.deltaY < 0) {
        const container = active.nodes.find(
          (node) => node.children.length && contains(node, world),
        );
        const child = container?.children
          .map((id) => scene.scopes[id])
          .find((scope) =>
            contains(
              {
                x: scope.local.x + scope.bounds.x * scope.local.zoom,
                y: scope.local.y + scope.bounds.y * scope.local.zoom,
                width: scope.bounds.width * scope.local.zoom,
                height: scope.bounds.height * scope.local.zoom,
              },
              world,
            ),
          );
        if (child && nextZoom * child.local.zoom >= 0.8) {
          studio.view(compose(next, child.local), child.id);
          return;
        }
      } else if (active.parent && nextZoom < 0.6) {
        studio.view(compose(next, inverse(active.local)), active.parent);
        return;
      }
      studio.view(next);
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => element.removeEventListener("wheel", wheel);
  }, [host, scene]);
}
