/** 与业务无关的局部逻辑坐标与视口契约。 */
export interface FlowPoint {
  readonly x: number;
  readonly y: number;
}
export interface FlowRect extends FlowPoint {
  readonly width: number;
  readonly height: number;
}
export interface ViewportTransform extends FlowPoint {
  readonly zoom: number;
}
export type FlowAnchorSide = "top" | "right" | "bottom" | "left";
export interface FlowNode<T = unknown> {
  readonly id: string;
  readonly position: FlowPoint;
  readonly size: { readonly width: number; readonly height: number };
  readonly data: T;
}
