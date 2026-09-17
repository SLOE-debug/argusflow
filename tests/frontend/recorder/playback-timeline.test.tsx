import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackTimeline } from "../../../src/components/recorder/playback/PlaybackTimeline";
import { overview } from "./playback-fixtures";

beforeEach(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.stubGlobal("PointerEvent", MouseEvent);
});
afterEach(() => vi.unstubAllGlobals());
function setup(toggle = vi.fn()) {
  const seek = vi.fn();
  render(
    <PlaybackTimeline
      timeline={overview}
      time={3000}
      playing={false}
      onMarker={vi.fn()}
      onSeek={seek}
      onPause={vi.fn()}
      onToggle={toggle}
      onRefresh={vi.fn()}
    />,
  );
  return seek;
}
describe("双轨时间线模式隔离", () => {
  it("选择时间段后仍能点击播放，并自动回到浏览模式", () => {
    const toggle = vi.fn();
    setup(toggle);
    fireEvent.click(screen.getByRole("button", { name: "选择时间段" }));
    const play = screen.getByRole("button", { name: "播放回看" });
    expect(play).toBeEnabled();
    fireEvent.click(play);
    expect(toggle).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "浏览画面" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      screen.queryByRole("slider", { name: "片段起点" }),
    ).not.toBeInTheDocument();
  });
  it("浏览模式完全移除选区与隐私控件，返回选择模式可继续编辑原选区", () => {
    setup();
    expect(screen.queryByText("已选择时间段")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "处理所选内容" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "选择时间段" }));
    expect(screen.getByRole("slider", { name: "片段起点" })).toHaveAttribute(
      "aria-valuenow",
      "3000",
    );
    expect(screen.getByRole("button", { name: "处理所选内容" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "调整时间" }));
    fireEvent.change(
      screen.getByRole("spinbutton", { name: "片段起点（秒）" }),
      { target: { value: "2.5" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "浏览画面" }));
    expect(screen.getAllByRole("slider")).toHaveLength(1);
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
    expect(screen.queryByText("已选择时间段")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "处理所选内容" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "选择时间段" }));
    expect(screen.getByRole("slider", { name: "片段起点" })).toHaveAttribute(
      "aria-valuenow",
      "2500",
    );
  });
  it("浏览拖动只定位时间；切换模式后相同拖动创建选区", () => {
    const seek = setup();
    const drag = () => {
      const lanes = screen.getByRole("group", { name: "事件轨道" });
      const track = lanes.parentElement!;
      vi.spyOn(track, "getBoundingClientRect").mockReturnValue({
        left: 100,
        width: 1000,
      } as DOMRect);
      track.setPointerCapture = vi.fn();
      fireEvent.pointerDown(lanes, { button: 0, clientX: 700 });
      fireEvent.pointerMove(track, { clientX: 900 });
      fireEvent.pointerUp(track, { clientX: 900 });
    };
    drag();
    expect(seek).toHaveBeenLastCalledWith(8000);
    expect(screen.getAllByRole("slider")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "选择时间段" }));
    drag();
    expect(screen.getByRole("slider", { name: "片段起点" })).toHaveAttribute(
      "aria-valuenow",
      "6000",
    );
    expect(screen.getByRole("slider", { name: "片段终点" })).toHaveAttribute(
      "aria-valuenow",
      "8000",
    );
  });
});
