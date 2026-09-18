import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Toast } from "../../../src/components/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Toast", () => {
  it("pauses dismissal during interaction and resumes after leaving", () => {
    vi.useFakeTimers();
    const close = vi.fn();
    render(
      <Toast message="已保存" type="success" duration={1000} onClose={close} />,
    );
    fireEvent.mouseEnter(screen.getByRole("status"));
    act(() => vi.advanceTimersByTime(2000));
    expect(close).not.toHaveBeenCalled();
    fireEvent.mouseLeave(screen.getByRole("status"));
    act(() => vi.advanceTimersByTime(1000));
    expect(close).toHaveBeenCalledOnce();
  });

  it("keeps actionable errors until explicitly dismissed", () => {
    vi.useFakeTimers();
    const close = vi.fn();
    render(
      <Toast
        message="保存失败"
        type="error"
        duration={0}
        onClose={close}
        actions={<span>恢复操作</span>}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain("恢复操作");
    act(() => vi.advanceTimersByTime(10000));
    expect(close).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "关闭提示" }));
    expect(close).toHaveBeenCalledOnce();
  });
});
