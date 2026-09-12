import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  buildCanvasScene,
  childScopes,
  createWorkflow,
  nodeById,
  studio,
} from "../../../../src/features/workflow";
import { addNode } from "../../../../src/features/workflow/model/graph";
import {
  compose,
  fitBounds,
  inverse,
  worldToScreen,
} from "../../../../src/flow";
import { hitTest } from "../../../../src/components/workflow/canvas/hitTest";
import { presentNodes } from "../../../../src/components/workflow/canvas/scene";
import {
  canvasEnvironment,
  installCanvas,
  localPoint,
  nestedFixture,
} from "../../support/canvasFixture";

beforeEach(() => {
  vi.useFakeTimers();
  canvasEnvironment();
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it.each(["if", "switch"])("%s 多分支的相同局部坐标命中各自子流程", (kind) => {
  const empty = createWorkflow();
  const created = addNode(empty, empty.definition.root, kind, { x: 80, y: 80 });
  let file = created.file;
  const branches = childScopes(nodeById(file, created.id)!.action);
  const ids: string[] = [];
  for (const branch of branches) {
    const child = addNode(file, branch.id, "wait", { x: 80, y: 80 });
    ids.push(child.id);
    file = child.file;
  }
  const scene = buildCanvasScene(file),
    nodes = presentNodes(file);
  const camera = { x: 30, y: 40, zoom: 1.4 };
  branches.forEach((branch, index) => {
    const point = worldToScreen(
      { x: 150, y: 120 },
      compose(camera, scene.scopes[branch.id].transform),
    );
    expect(hitTest(scene, nodes, camera, point)).toMatchObject({
      kind: "node",
      scope: branch.id,
      node: ids[index],
    });
  });
  expect(
    Object.values(scene.details).every((item) => item.endpoints.length === 2),
  ).toBe(true);
});

it("空循环体在所属容器内添加首个节点，不能覆盖成根流程", () => {
  const empty = createWorkflow();
  const created = addNode(empty, empty.definition.root, "while", {
    x: 80,
    y: 100,
  });
  const scope = childScopes(nodeById(created.file, created.id)!.action)[0].id;
  const { host } = installCanvas(created.file);
  const geometry = buildCanvasScene(created.file).scopes[scope];
  act(() =>
    studio.view(
      compose(
        fitBounds(geometry.bounds, 1200, 800),
        inverse(geometry.transform),
      ),
    ),
  );
  fireEvent.doubleClick(host, localPoint(scope, { x: 120, y: 70 }));
  const input = screen.getByRole("textbox", { name: "搜索节点" });
  fireEvent.change(input, { target: { value: "声明变量" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(studio.active!.scope).toBe(scope);
  expect(studio.active!.file.definition.scopes[0].nodes).toHaveLength(1);
  expect(
    studio.active!.file.definition.scopes.find((item) => item.id === scope)!
      .nodes,
  ).toHaveLength(1);
});

it("绘图上下文不可用时显示画布错误，外部视图控件仍可使用", () => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  installCanvas(nestedFixture().file);
  expect(screen.getByRole("alert")).toHaveTextContent("画布无法显示");
  expect(screen.getByRole("button", { name: "显示全部" })).toBeEnabled();
});
