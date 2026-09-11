import { vi } from "vitest";

/** jsdom 的布局与原生 dialog 替身；仅用于 DOM 行为测试。 */
export function mockUiEnvironment(): void {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
      unobserve() {}
    },
  );
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    left: 20,
    top: 30,
    right: 180,
    bottom: 62,
    width: 160,
    height: 32,
    x: 20,
    y: 30,
    toJSON: () => ({}),
  });
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value() {
      this.setAttribute("open", "");
    },
  });
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value() {
      this.removeAttribute("open");
    },
  });
}
