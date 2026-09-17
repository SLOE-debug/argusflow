import { expect, it } from "vitest";
import { distributeNodes, type FlowNode } from "../../../src/flow";

it.each(["horizontal", "vertical"] as const)(
  "%s 分布按不同尺寸节点的边缘均分空隙，保留首尾和未选中节点",
  (mode) => {
    const nodes: FlowNode[] = [152, 208, 424, 152].map((size, index) => ({
      id: String(index),
      position: {
        x: [0, 320, 568, 1064][index],
        y: [0, 320, 568, 1064][index],
      },
      size: { width: size, height: size },
      data: null,
    }));
    const outside = { ...nodes[0], id: "outside" };
    const result = distributeNodes(
      [...nodes, outside],
      new Set(nodes.map((node) => node.id)),
      mode,
    );
    const axis = mode === "horizontal" ? "x" : "y";
    const other = mode === "horizontal" ? "y" : "x";
    const dimension = mode === "horizontal" ? "width" : "height";
    for (let index = 1; index < nodes.length; index++) {
      expect(
        result[index].position[axis] -
          result[index - 1].position[axis] -
          result[index - 1].size[dimension],
      ).toBeCloseTo(280 / 3);
      expect(result[index].position[other]).toBe(nodes[index].position[other]);
    }
    expect(result[0]).toBe(nodes[0]);
    expect(result[3].position[axis]).toBeCloseTo(nodes[3].position[axis]);
    expect(result[4]).toBe(outside);
  },
);

it("空间不足时消除重叠；少于三个节点不改变排列", () => {
  const nodes: FlowNode[] = [0, 20, 40].map((x) => ({
    id: String(x),
    position: { x, y: 0 },
    size: { width: 100, height: 60 },
    data: null,
  }));
  const selected = new Set(nodes.map((node) => node.id));
  expect(
    distributeNodes(nodes, selected, "horizontal").map(
      (node) => node.position.x,
    ),
  ).toEqual([0, 100, 200]);
  expect(distributeNodes(nodes, new Set(["0", "20"]), "horizontal")).toBe(
    nodes,
  );
});
