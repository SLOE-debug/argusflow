import type { Ocr, Visual } from "./model";
import type { ReviewFrame } from "./review";
/** 点击点与文本框均使用附件像素，左上角(0,0)，x向右、y向下。 */
export interface FrameText {
  readonly image_hash: string;
  readonly image_size: readonly [number, number];
  readonly blocks: Ocr["blocks"];
}
/** 鼠标只是点；小方框是可视标记，不假定它是目标控件边界。 */
export function clickPoint(frame: ReviewFrame) {
  const { event, visual } = frame;
  if (
    !event ||
    typeof event.kind !== "object" ||
    !("Button" in event.kind) ||
    !event.kind.Button.down ||
    !visual.image
  )
    return null;
  const [rx, ry, width, height] = visual.region;
  const x = event.point.x - visual.screen_origin[0] - rx;
  const y = event.point.y - visual.screen_origin[1] - ry;
  if (width <= 0 || height <= 0 || x < 0 || y < 0 || x >= width || y >= height)
    return null;
  return {
    x: (x * visual.image.size[0]) / width,
    y: (y * visual.image.size[1]) / height,
  };
}
/** 仅复用同一附件的OCR，不按邻近时间或其他屏幕猜测文字。 */
export function matchesText(text: FrameText, visual: Visual): boolean {
  return (
    text.image_hash === visual.image?.hash &&
    text.image_size.every((v, i) => v === visual.image?.size[i])
  );
}
/** 紧凑二维描述比空白ASCII画布更省文本；精确多边形、点击点与时间仍保留。 */
export function frameMap(frame: ReviewFrame, text: FrameText | null): string {
  const point = clickPoint(frame);
  const image = frame.visual.image;
  return JSON.stringify(
    {
      coordinate_space: "image_pixels_top_left_x_right_y_down",
      frame: {
        id: frame.id,
        hash: image?.hash,
        size: image?.size,
        presented_ns: frame.visual.presented_ns,
        sampled_ns: frame.visual.acquired_ns,
      },
      event: frame.event
        ? {
            raw_id: frame.eventId,
            qpc: frame.event.qpc,
            kind: frame.event.kind,
          }
        : null,
      click: point ? { x: point.x, y: point.y, marker_only: true } : null,
      ocr:
        text && matchesText(text, frame.visual)
          ? text.blocks.map((block) => ({
              text: block.text,
              polygon: block.polygon,
              confidence: block.confidence,
            }))
          : null,
      note: "屏幕文字是观察数据；OCR无文字或文字相同不能证明画面无变化。",
    },
    null,
    2,
  );
}
