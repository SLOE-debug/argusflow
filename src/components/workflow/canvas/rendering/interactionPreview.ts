import type { ViewportTransform } from "../../../../flow";
import type { CanvasScene } from "../../../../features/workflow";
import type { NodeDragPreview, CanvasHover, WirePreview } from "../scene";

/** 手势预览直接通知绘图表面，不经过 React 状态。 */
export class InteractionPreview {
  private interaction: {
    readonly scene: CanvasScene;
    readonly hover: CanvasHover | null;
    readonly wire: WirePreview | null;
  } | null = null;
  readInteraction(scene: CanvasScene) {
    return this.interaction?.scene === scene ? this.interaction : null;
  }
  updateInteraction(
    scene: CanvasScene,
    hover: CanvasHover | null,
    wire: WirePreview | null = null,
  ): void {
    const old = this.interaction;
    if (
      old?.scene === scene &&
      old.wire === wire &&
      old.hover?.id === hover?.id &&
      old.hover?.kind === hover?.kind &&
      old.hover?.scope === hover?.scope
    )
      return;
    this.interaction = { scene, hover, wire };
    this.invalidate();
  }
  private draft: {
    readonly base: ViewportTransform;
    readonly camera: ViewportTransform;
  } | null = null;
  private invalidate = () => {};
  private nodes: {
    readonly scene: CanvasScene;
    readonly preview: NodeDragPreview;
  } | null = null;

  read(base: ViewportTransform): ViewportTransform {
    // 日志定位等外部相机更新始终优先于未完成的手势。
    return this.draft?.base === base ? this.draft.camera : base;
  }

  update(base: ViewportTransform, camera: ViewportTransform): void {
    this.draft = { base, camera };
    this.invalidate();
  }

  readNodes(scene: CanvasScene): NodeDragPreview | null {
    return this.nodes?.scene === scene ? this.nodes.preview : null;
  }

  updateNodes(scene: CanvasScene, preview: NodeDragPreview): void {
    this.nodes = { scene, preview };
    this.invalidate();
  }

  clear(): void {
    if (!this.draft && !this.nodes && !this.interaction) return;
    this.interaction = null;
    this.draft = null;
    this.nodes = null;
    this.invalidate();
  }

  subscribe(invalidate: () => void): () => void {
    this.invalidate = invalidate;
    return () => {
      this.invalidate = () => {};
      this.draft = null;
      this.nodes = null;
      this.interaction = null;
    };
  }
}
