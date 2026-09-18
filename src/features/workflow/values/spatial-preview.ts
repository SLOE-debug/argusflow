import type { Value, ValueType } from "../model/contracts";
import { isValue } from "./validation";
const floats: ValueType = { type: "list", of: { type: "float" } };
const optional = (of: ValueType): ValueType => ({ type: "optional", of });
export const SPATIAL_PREVIEW_TYPE: ValueType = {
  type: "record",
  of: {
    kind: { type: "text" },
    items: {
      type: "list",
      of: {
        type: "record",
        of: {
          space: { type: "text" },
          scope: floats,
          anchor: floats,
          angles: optional(floats),
          candidates: {
            type: "list",
            of: {
              type: "record",
              of: {
                node: { type: "int" },
                bounds: floats,
                angle: optional({ type: "float" }),
                distance: { type: "float" },
                reason: optional({ type: "text" }),
                rank: optional({ type: "int" }),
                selected: { type: "bool" },
              },
            },
          },
        },
      },
    },
  },
};
/** 检查语义标记；普通记录同样合法，false 不应排除整个记录类型。 */
export function isSpatialPreview(value: Value): boolean {
  return (
    value.type === "record" &&
    value.value.kind?.type === "text" &&
    value.value.kind.value === "spatial_preview"
  );
}
function unwrap(value: Value, expected: ValueType): unknown {
  if (value.type !== expected.type) throw new Error("预览字段类型不一致");
  switch (value.type) {
    case "record": {
      if (expected.type !== "record") throw new Error("预览记录类型无效");
      return Object.fromEntries(
        Object.entries(expected.of).map(([k, t]) => [
          k,
          unwrap(value.value[k], t),
        ]),
      );
    }
    case "list":
    case "optional": {
      if (expected.type !== "list" && expected.type !== "optional")
        throw new Error("预览容器类型无效");
      return value.type === "list"
        ? value.value.map((v) => unwrap(v, expected.of))
        : value.value === null
          ? null
          : unwrap(value.value, expected.of);
    }
    case "int":
      return Number(value.value);
    default:
      return value.value;
  }
}
type Rect = readonly [number, number, number, number];
interface Candidate {
  readonly node: number;
  readonly bounds: Rect;
  readonly angle: number | null;
  readonly distance: number;
  readonly reason: string | null;
  readonly rank: number | null;
  readonly selected: boolean;
}
export interface Preview {
  readonly space: string;
  readonly scope: Rect;
  readonly anchor: Rect;
  readonly angles: readonly [number, number] | null;
  readonly candidates: readonly Candidate[];
}
const record = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const finite = (v: unknown): v is number =>
  typeof v === "number" && Number.isFinite(v);
const rect = (v: unknown): v is Rect =>
  Array.isArray(v) && v.length === 4 && v.every(finite) && v[2] > 0 && v[3] > 0;

/** 运行输出是外部数据，验证后才能用于 SVG 坐标。 */
export function parseSpatialPreview(source: Value): readonly Preview[] | null {
  try {
    if (!isValue(source) || !isSpatialPreview(source)) return null;
    const decoded = unwrap(source, SPATIAL_PREVIEW_TYPE);
    if (!record(decoded)) return null;
    const value: unknown = decoded.items;
    if (!Array.isArray(value) || value.length > 64) return null;
    for (const p of value) {
      if (
        !record(p) ||
        typeof p.space !== "string" ||
        !rect(p.scope) ||
        !rect(p.anchor) ||
        !(
          p.angles === null ||
          (Array.isArray(p.angles) &&
            p.angles.length === 2 &&
            p.angles.every(finite))
        ) ||
        !Array.isArray(p.candidates) ||
        p.candidates.length > 10000
      )
        return null;
      for (const c of p.candidates) {
        if (
          !record(c) ||
          !Number.isSafeInteger(c.node) ||
          Number(c.node) < 0 ||
          !rect(c.bounds) ||
          !finite(c.distance) ||
          c.distance < 0 ||
          !(c.angle === null || finite(c.angle)) ||
          !(c.reason === null || typeof c.reason === "string") ||
          !(
            c.rank === null ||
            (Number.isSafeInteger(c.rank) && Number(c.rank) > 0)
          ) ||
          typeof c.selected !== "boolean"
        )
          return null;
      }
    }
    return value as Preview[];
  } catch {
    return null;
  }
}
