import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NodeTree } from "../../../src/components/workflow/palette/NodeTree";
import { NodeSearch } from "../../../src/components/workflow/palette/NodeSearch";
import { CanvasMenu } from "../../../src/components/workflow/canvas/CanvasMenu";
import { presentNodes } from "../../../src/components/workflow/canvas/scene";
import {
  createWorkflow,
  nodeUsage,
  studio,
} from "../../../src/features/workflow";
import { INITIAL_STATE } from "../../../src/features/workflow/studio/state";
import { addNode } from "../../../src/features/workflow/model/graph";
import { mockUiEnvironment } from "../support/ui";

beforeEach(() => {
  vi.useFakeTimers();
  mockUiEnvironment();
  studio.store.setState({
    ...INITIAL_STATE,
    initialization: { status: "ready" },
  });
  nodeUsage.store.setState({ entries: {}, error: null });
  vi.spyOn(studio.api, "save").mockImplementation(async (file) => ({
    file,
    revision: "saved",
  }));
});
afterEach(async () => {
  cleanup();
  await studio.flushAll();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it.each([
  ["start", "开始", "nodeStart", "node-start"],
  ["end", "结束", "nodeEnd", "node-end"],
  ["let", "声明变量", "data", "data"],
  ["if", "条件分支", "structure", "structure"],
])(
  "%s 在节点树、搜索、右键菜单和画布中保持同一语义颜色",
  (kind, title, tone, cssTone) => {
    studio.create();
    const tab = studio.active!;
    const tree = render(<NodeTree tab={tab} query="" />);
    const treeIcon = screen
      .getByRole("treeitem", { name: title })
      .querySelector("svg")!;
    expect(treeIcon.classList.contains("text-" + cssTone)).toBe(true);
    expect(treeIcon).toHaveAttribute("fill", "none");
    expect(treeIcon).toHaveAttribute("stroke-width", "1.7");
    const shape = treeIcon.innerHTML;
    tree.unmount();
    const search = render(<NodeSearch onPick={vi.fn()} onClose={vi.fn()} />);
    const searchIcon = screen
      .getByRole("button", { name: new RegExp("^" + title + " ") })
      .querySelector("svg")!;
    expect(searchIcon.classList.contains("text-" + cssTone)).toBe(true);
    expect(searchIcon.innerHTML).toBe(shape);
    search.unmount();
    render(
      <CanvasMenu
        tab={tab}
        menu={{ x: 0, y: 0, scope: tab.scope, point: { x: 0, y: 0 } }}
        onClose={vi.fn()}
        onAdd={vi.fn()}
      />,
    );
    fireEvent.mouseEnter(screen.getByRole("menuitem", { name: "添加节点" }));
    const menuIcon = screen
      .getByRole("menuitem", { name: title })
      .querySelector("svg")!;
    expect(menuIcon.classList.contains("text-" + cssTone)).toBe(true);
    expect(menuIcon.innerHTML).toBe(shape);
    const empty = createWorkflow();
    const added = addNode(empty, empty.definition.root, kind, { x: 0, y: 0 });
    expect(presentNodes(added.file).get(added.id)?.tone).toBe(tone);
  },
);
