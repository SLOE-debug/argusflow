import { SpatialPreview } from "../../../src/components/workflow/data/SpatialPreview";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { spatialViewport } from "../../../src/components/workflow/data/SpatialPlot";
import {
  parseSpatialPreview,
  isSpatialPreview,
  SPATIAL_PREVIEW_TYPE,
} from "../../../src/features/workflow/values/spatial-preview";
import type {
  Value,
  ValueType,
} from "../../../src/features/workflow/model/contracts";
function encode(value: unknown, type: ValueType): Value {
  if (value === undefined) return { type: "text", value: "invalid" };
  if (type.type === "record")
    return {
      type: "record",
      value: Object.fromEntries(
        Object.entries(type.of).map(([k, t]) => [
          k,
          encode((value as Record<string, unknown>)[k], t),
        ]),
      ),
    };
  if (type.type === "list")
    return {
      type: "list",
      value: (value as unknown[]).map((v) => encode(v, type.of)),
    };
  if (type.type === "optional")
    return {
      type: "optional",
      value: value === null ? null : encode(value, type.of),
    };
  if (type.type === "int") return { type: "int", value: String(value) };
  if (type.type === "float") return { type: "float", value: value as number };
  if (type.type === "bool") return { type: "bool", value: value as boolean };
  return { type: "text", value: value as string };
}
function output(items: unknown): Value {
  return encode({ kind: "spatial_preview", items }, SPATIAL_PREVIEW_TYPE);
}
const sample = [
  {
    space: "window:1",
    scope: [0, 0, 800, 600],
    anchor: [20, 20, 40, 20],
    angles: [337.5, 22.5],
    candidates: [
      {
        node: 2,
        bounds: [120, 20, 40, 20],
        angle: 0,
        distance: 100,
        reason: null,
        rank: 1,
        selected: true,
      },
    ],
  },
];
describe("空间定位预览", () => {
  it("按语义标记识别任意命名的输出并保留类型检查", () => {
    const outputs = {
      renamed: output(sample),
      ordinary: { type: "text", value: "spatial_preview" } as Value,
    };
    expect(Object.values(outputs).filter(isSpatialPreview)).toHaveLength(1);
    expect(parseSpatialPreview(outputs.renamed)).toEqual(sample);
    expect(
      parseSpatialPreview(
        output([
          {
            ...sample[0],
            candidates: [{ ...sample[0].candidates[0], node: -1 }],
          },
        ]),
      ),
    ).toBeNull();
  });
  it("拒绝损坏数据和无效外框", () => {
    expect(parseSpatialPreview({ type: "text", value: "not json" })).toBeNull();
    expect(
      parseSpatialPreview(output([{ ...sample[0], scope: [0, 0, 0, 600] }])),
    ).toBeNull();
    expect(
      parseSpatialPreview(
        output([{ ...sample[0], candidates: [{ node: 2 }] }]),
      ),
    ).toBeNull();
  });
  it("绘制扇形、锚点和候选并显示距离与排名", () => {
    render(<SpatialPreview previews={parseSpatialPreview(output(sample))!} />);
    expect(
      screen.getByRole("img", { name: "空间定位预览" }),
    ).toBeInTheDocument();
    expect(screen.getByText("100.0")).toBeInTheDocument();
    expect(screen.getByText("选中")).toBeInTheDocument();
  });
  it("默认围绕对象取景，完整范围由用户显式切换", () => {
    const preview = parseSpatialPreview(output(sample))![0];
    const [x, y, width, height] = spatialViewport(preview);
    expect(width).toBeLessThan(preview.scope[2]);
    expect(height).toBeLessThan(preview.scope[3]);
    for (const [left, top, w, h] of [
      preview.anchor,
      ...preview.candidates.map((item) => item.bounds),
    ]) {
      expect(x).toBeLessThan(left);
      expect(y).toBeLessThan(top);
      expect(x + width).toBeGreaterThan(left + w);
      expect(y + height).toBeGreaterThan(top + h);
    }
    render(<SpatialPreview previews={[preview]} />);
    const plot = screen.getByRole("img", { name: "空间定位预览" });
    expect(plot).toHaveAttribute("viewBox", spatialViewport(preview).join(" "));
    fireEvent.click(screen.getByRole("button", { name: "显示整个范围" }));
    expect(plot).toHaveAttribute("viewBox", "0 0 800 600");
    fireEvent.click(screen.getByRole("button", { name: "放大目标区域" }));
    expect(plot).toHaveAttribute("viewBox", spatialViewport(preview).join(" "));
  });
});
