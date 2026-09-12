import { useEffect, useRef, useState, type RefObject } from "react";
import { drawScene, type SceneFrame } from "./drawScene";
import { DrawingResources } from "./resources";
import type { InteractionPreview } from "./interactionPreview";

/** 管理单个绘图表面的生命周期；同一帧内的更新只绘制一次。 */
export function useCanvasRenderer(
  canvas: RefObject<HTMLCanvasElement | null>,
  frame: Omit<SceneFrame, "preview" | "wire" | "hover">,
  preview: InteractionPreview,
): string | null {
  const latest = useRef(frame);
  latest.current = frame;
  const invalidate = useRef<() => void>(() => {});
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const context = element.getContext("2d");
    if (!context) {
      setError("画布无法显示，请重新打开此工作流。");
      return;
    }
    let pending: number | null = null;
    let disposed = false;
    const schedule = () => {
      if (disposed || pending !== null) return;
      pending = requestAnimationFrame(() => {
        pending = null;
        const next = latest.current;
        const ratio = window.devicePixelRatio || 1;
        const width = Math.max(1, Math.round(next.width * ratio));
        const height = Math.max(1, Math.round(next.height * ratio));
        if (element.width !== width || element.height !== height) {
          element.width = width;
          element.height = height;
        }
        try {
          context.setTransform(ratio, 0, 0, ratio, 0, 0);
          drawScene(context, resources, {
            ...next,
            camera: preview.read(next.camera),
            preview: preview.readNodes(next.scene),
            hover: preview.readInteraction(next.scene)?.hover ?? null,
            wire: preview.readInteraction(next.scene)?.wire ?? null,
          });
          setError(null);
        } catch (cause) {
          console.error("画布绘制失败", cause);
          setError("画布绘制失败，请重新打开此工作流。");
        }
      });
    };
    const resources = new DrawingResources(schedule);
    const fontsChanged = () => {
      resources.clearText();
      schedule();
    };
    // 监听当前 DPI 的媒体查询；移动到另一显示器后重新订阅新倍率。
    let dpi: MediaQueryList | undefined;
    const dpiChanged = () => {
      dpi?.removeEventListener("change", dpiChanged);
      dpi = window.matchMedia?.(
        `(resolution: ${window.devicePixelRatio || 1}dppx)`,
      );
      dpi?.addEventListener("change", dpiChanged);
      schedule();
    };
    invalidate.current = schedule;
    const unsubscribe = preview.subscribe(schedule);
    dpiChanged();
    document.fonts?.addEventListener("loadingdone", fontsChanged);
    window.addEventListener("resize", schedule);
    return () => {
      disposed = true;
      unsubscribe();
      if (pending !== null) cancelAnimationFrame(pending);
      invalidate.current = () => {};
      resources.dispose();
      dpi?.removeEventListener("change", dpiChanged);
      document.fonts?.removeEventListener("loadingdone", fontsChanged);
      window.removeEventListener("resize", schedule);
    };
  }, [canvas, preview]);
  useEffect(() => {
    invalidate.current();
  }, [frame]);
  return error;
}
