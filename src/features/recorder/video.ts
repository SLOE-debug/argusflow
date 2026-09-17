import type { Attachment } from "./model";
/** 时间线请求保留递增身份，重复定位同一时刻仍可重新读取。 */
export interface VideoSeek {
  readonly qpc: string;
  readonly serial: number;
  readonly direction?: number;
  readonly markerId?: number;
}
export interface VideoFrame {
  readonly image: Attachment;
  readonly url: string;
  readonly at_qpc: string;
  readonly presented_qpc: string;
  readonly acquired_qpc: string;
  readonly repeated: boolean;
  readonly segment: string;
  readonly sequence: number;
  readonly screen_origin: readonly [number, number];
  readonly dpi: readonly [number, number];
  readonly qpc_frequency: number;
}
