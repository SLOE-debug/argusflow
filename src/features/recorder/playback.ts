/** 总览独立于分页操作列表，长录制不会因列表淘汰而丢失轨道标记。 */
export interface VideoTimeline {
  readonly origin_qpc: string;
  readonly frequency: number;
  readonly duration_ms: number;
  readonly segments: readonly {
    readonly start_ms: number;
    readonly end_ms: number;
  }[];
  readonly markers: readonly InputMarker[];
  readonly tail: string | null;
}
export interface InputMarker {
  readonly id: number;
  readonly time_ms: number;
  readonly input:
    | { readonly kind: "key"; readonly vk: number }
    | {
        readonly kind: "button";
        readonly button:
          "Left" | "Right" | "Middle" | { readonly Extra: number };
      }
    | {
        readonly kind: "wheel";
        readonly horizontal: boolean;
        readonly delta: number;
      };
}
/** 会话相对时间与原始QPC转换，仅差值进入浮点毫秒域。 */
export const timeQpc = (timeline: VideoTimeline, ms: number) =>
  (
    BigInt(timeline.origin_qpc) +
    BigInt(Math.round((ms * timeline.frequency) / 1000))
  ).toString();
export const qpcTime = (timeline: VideoTimeline, qpc: string) =>
  (Number(BigInt(qpc) - BigInt(timeline.origin_qpc)) * 1000) /
  timeline.frequency;
export function playbackTime(ms: number) {
  const value = Math.max(0, Math.round(ms));
  return `${String(Math.floor(value / 60000)).padStart(2, "0")}:${String(Math.floor(value / 1000) % 60).padStart(2, "0")}.${String(value % 1000).padStart(3, "0")}`;
}
export function markerLabel(marker: InputMarker) {
  const input = marker.input;
  if (input.kind === "key")
    return `按键 ${input.vk >= 65 && input.vk <= 90 ? String.fromCharCode(input.vk) : (({ 13: "Enter", 27: "Esc", 32: "Space", 91: "Win", 92: "Win" } as Readonly<Record<number, string>>)[input.vk] ?? `VK ${input.vk}`)}`;
  if (input.kind === "wheel")
    return `${input.horizontal ? "水平" : "垂直"}滚动 ${input.delta}`;
  return `${typeof input.button === "string" ? { Left: "左键", Right: "右键", Middle: "中键" }[input.button] : "侧键"}按下`;
}
/** 单轨按屏幕列聚合可视标记，保留两轨并行事件，原始标记不变。 */
export function timelineMarkers(timeline: VideoTimeline) {
  const columns = new Map<string, InputMarker>();
  for (const marker of timeline.markers) {
    if (marker.time_ms < 0 || marker.time_ms > timeline.duration_ms) continue;
    const lane = marker.input.kind === "key" ? 0 : 1;
    columns.set(
      `${lane}:${Math.round((marker.time_ms / Math.max(1, timeline.duration_ms)) * 1200)}`,
      marker,
    );
  }
  return [...columns.values()].map((marker) => ({
    id: String(marker.id),
    value: marker.time_ms,
    label: `${playbackTime(marker.time_ms)} · ${markerLabel(marker)}`,
    lane: marker.input.kind === "key" ? 0 : 1,
    tone:
      marker.input.kind === "key" ? ("accent" as const) : ("neutral" as const),
  }));
}
