import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Menu, type MenuItem } from "../../../src/components/ui";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/** 同时覆盖普通项、禁用项与级联项的菜单夹具。 */
function items(action: () => void): readonly MenuItem[] {
  return [
    { type: "action", label: "不可用", disabled: true, action },
    { type: "action", label: "复制", action },
    { type: "separator", id: "group" },
    {
      type: "submenu",
      label: "添加节点",
      items: [
        { type: "action", label: "等待", action },
        { type: "action", label: "声明变量", action },
      ],
    },
  ];
}

it("方向键在当前菜单循环，进入子菜单后 Escape 返回触发项", () => {
  const close = vi.fn(),
    action = vi.fn();
  render(
    <Menu
      x={100}
      y={100}
      label="画布菜单"
      items={items(action)}
      onClose={close}
    />,
  );
  const root = screen.getByRole("menu", { name: "画布菜单" });
  expect(screen.getByRole("menuitem", { name: "复制" })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
  const trigger = screen.getByRole("menuitem", { name: "添加节点" });
  expect(trigger).toHaveFocus();
  fireEvent.keyDown(trigger, { key: "ArrowRight" });
  expect(trigger).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByRole("menuitem", { name: "等待" })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: "End" });
  expect(screen.getByRole("menuitem", { name: "声明变量" })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(screen.queryByRole("menu", { name: "添加节点" })).toBeNull();
  expect(trigger).toHaveFocus();
  expect(close).not.toHaveBeenCalled();
  fireEvent.keyDown(root, { key: "Escape" });
  expect(close).toHaveBeenCalledTimes(1);
  expect(action).not.toHaveBeenCalled();
});

it("悬停展开，子菜单点击只执行一次，外部点击关闭整组菜单", () => {
  const close = vi.fn(),
    action = vi.fn();
  render(
    <Menu
      x={100}
      y={100}
      label="画布菜单"
      items={items(action)}
      onClose={close}
    />,
  );
  fireEvent.mouseEnter(screen.getByRole("menuitem", { name: "添加节点" }));
  const child = screen.getByRole("menu", { name: "添加节点" });
  const wait = within(child).getByRole("menuitem", { name: "等待" });
  fireEvent.pointerDown(wait);
  expect(close).not.toHaveBeenCalled();
  fireEvent.click(wait);
  expect(action).toHaveBeenCalledTimes(1);
  expect(close).toHaveBeenCalledTimes(1);
  fireEvent.mouseEnter(screen.getByRole("menuitem", { name: "复制" }));
  expect(screen.queryByRole("menu", { name: "添加节点" })).toBeNull();
  fireEvent.click(screen.getByRole("menuitem", { name: "不可用" }));
  expect(action).toHaveBeenCalledTimes(1);
  fireEvent.pointerDown(document.body);
  expect(close).toHaveBeenCalledTimes(2);
});

it("根菜单避让视口四边，右侧空间不足时子菜单向左展开", () => {
  vi.stubGlobal("innerWidth", 1024);
  vi.stubGlobal("innerHeight", 768);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
    function (this: HTMLElement) {
      return this.getAttribute("role") === "menu"
        ? new DOMRect(0, 0, 192, 200)
        : new DOMRect(840, 740, 176, 28);
    },
  );
  const props = { label: "画布菜单", items: items(vi.fn()), onClose: vi.fn() };
  const { rerender } = render(<Menu {...props} x={1020} y={764} />);
  expect(screen.getByRole("menu", { name: "画布菜单" })).toHaveStyle({
    left: "824px",
    top: "560px",
  });
  fireEvent.mouseEnter(screen.getByRole("menuitem", { name: "添加节点" }));
  expect(screen.getByRole("menu", { name: "添加节点" })).toHaveStyle({
    left: "644px",
    top: "560px",
  });
  rerender(<Menu {...props} x={-50} y={-50} />);
  expect(screen.getByRole("menu", { name: "画布菜单" })).toHaveStyle({
    left: "8px",
    top: "8px",
  });
});
