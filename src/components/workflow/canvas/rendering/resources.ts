import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NodeIcon } from "../../presentation/NodeIcon";

/** 单个画布拥有的测量和图标缓存；卸载时解除图片回调。 */
export class DrawingResources {
  private readonly labels = new Map<string, string>();
  private readonly icons = new Map<string, HTMLImageElement>();
  constructor(private readonly invalidate: () => void) {}

  /** 按字体与可用宽度缓存省略文本，避免平移缩放重复测量。 */
  label(
    context: CanvasRenderingContext2D,
    text: string,
    width: number,
  ): string {
    const key = `${context.font}:${width}:${text}`;
    const cached = this.labels.get(key);
    if (cached !== undefined) return cached;
    let result = text;
    if (context.measureText(text).width > width) {
      const characters = Array.from(text);
      let low = 0,
        high = characters.length;
      while (low < high) {
        const middle = Math.ceil((low + high) / 2);
        if (
          context.measureText(characters.slice(0, middle).join("") + "…")
            .width <= width
        )
          low = middle;
        else high = middle - 1;
      }
      result = characters.slice(0, low).join("") + "…";
    }
    // 编辑文本可能持续变化，缓存设置上限以避免长会话持续增长。
    if (this.labels.size >= 4096) this.labels.clear();
    this.labels.set(key, result);
    return result;
  }

  /** SVG 仅作为离屏图标资源，场景没有节点 SVG/DOM。 */
  icon(
    context: CanvasRenderingContext2D,
    kind: string,
    color: string,
    x: number,
    y: number,
  ): void {
    const key = kind + color;
    let image = this.icons.get(key);
    if (!image) {
      if (this.icons.size >= 256) {
        this.icons.forEach((cached) => {
          cached.onload = null;
        });
        this.icons.clear();
      }
      image = new Image();
      image.onload = this.invalidate;
      const svg = renderToStaticMarkup(
        createElement(NodeIcon, { kind, size: 24 }),
      ).replace("<svg", `<svg color="${color}"`);
      image.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
      this.icons.set(key, image);
    }
    if (image.complete && image.naturalWidth)
      context.drawImage(image, x, y, 24, 24);
  }

  /** 字体加载完成后使测量结果失效。 */
  clearText(): void {
    this.labels.clear();
  }
  dispose(): void {
    this.icons.forEach((image) => {
      image.onload = null;
    });
    this.icons.clear();
    this.labels.clear();
  }
}
