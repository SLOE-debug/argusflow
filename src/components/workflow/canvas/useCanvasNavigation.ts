import { useCallback, useEffect, useRef, type RefObject } from "react";
import {
  compose,
  fitBounds,
  inverse,
  zoomAt,
  type FlowRect,
  type ViewportTransform,
} from "../../../flow";
import { studio, type CanvasScene } from "../../../features/workflow";

/** 单个根场景相机，缩放不会改变编辑作用域。 */
export function useCanvasNavigation(
  host: RefObject<HTMLDivElement | null>,
  scene: CanvasScene,
  size: { readonly width: number; readonly height: number },
) {
  const animation = useRef<number | null>(null);
  const cancel = useCallback(() => {
    if (animation.current !== null) cancelAnimationFrame(animation.current);
    animation.current = null;
  }, []);
  const animate = useCallback(
    (target: ViewportTransform) => {
      cancel();
      const tab = studio.active;
      if (!tab) return;
      const start = tab.viewport;
      const began = performance.now();
      let expected = start;
      const tick = (now: number) => {
        // 外部定位或用户手势接管相机时，旧动画立即退出。
        if (
          studio.active?.file.id !== tab.file.id ||
          studio.active.viewport !== expected
        ) {
          animation.current = null;
          return;
        }
        const progress = Math.min(1, (now - began) / 180);
        const t = 1 - (1 - progress) ** 3;
        expected = {
          x: start.x + (target.x - start.x) * t,
          y: start.y + (target.y - start.y) * t,
          zoom: Math.exp(
            Math.log(start.zoom) +
              (Math.log(target.zoom) - Math.log(start.zoom)) * t,
          ),
        };
        studio.view(expected);
        animation.current = progress < 1 ? requestAnimationFrame(tick) : null;
      };
      if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches)
        studio.view(target);
      else animation.current = requestAnimationFrame(tick);
    },
    [cancel],
  );
  const focus = useCallback(
    (id: string, bounds?: FlowRect) => {
      const scope = scene.scopes[id];
      if (!scope) return;
      studio.select([], id);
      animate(
        compose(
          fitBounds(bounds ?? scope.bounds, size.width, size.height),
          inverse(scope.transform),
        ),
      );
    },
    [scene, size.width, size.height, animate],
  );
  const zoom = useCallback(
    (factor: number) => {
      cancel();
      const camera = studio.active?.viewport;
      if (camera)
        studio.view(
          zoomAt(
            camera,
            { x: size.width / 2, y: size.height / 2 },
            Math.max(0.000001, Math.min(scene.maxZoom, camera.zoom * factor)),
          ),
        );
    },
    [cancel, scene.maxZoom, size.width, size.height],
  );
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      cancel();
      const camera = studio.active?.viewport;
      if (!camera) return;
      const rect = element.getBoundingClientRect();
      const delta =
        event.deltaY *
        (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? size.height : 1);
      const nextZoom = Math.max(
        0.000001,
        Math.min(
          scene.maxZoom,
          camera.zoom * Math.exp(-delta * (event.ctrlKey ? 0.01 : 0.0015)),
        ),
      );
      studio.view(
        zoomAt(
          camera,
          { x: event.clientX - rect.left, y: event.clientY - rect.top },
          nextZoom,
        ),
      );
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => {
      element.removeEventListener("wheel", wheel);
      cancel();
    };
  }, [host, scene.maxZoom, size.height, cancel]);
  useEffect(() => cancel, [cancel]);
  return { focus, zoom, cancel };
}
