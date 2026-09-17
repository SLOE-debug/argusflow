import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { PlaybackWorkspace } from "../../../src/components/recorder/playback/PlaybackWorkspace";
import { recorderApi } from "../../../src/features/recorder/api";
import { timeQpc } from "../../../src/features/recorder/playback";
import type { VideoFrame } from "../../../src/features/recorder/video";
import { clickAction, click } from "./review-fixtures";
import { overview } from "./playback-fixtures";
vi.mock("../../../src/features/recorder/api", () => ({
  recorderApi: {
    context: vi.fn(),
    videoTimeline: vi.fn(),
    videoFrame: vi.fn(),
  },
}));
afterEach(() => vi.unstubAllGlobals());
it("拖动时间线读取对应操作信息与视频，移除鼠标标记", async () => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.mocked(recorderApi.context).mockResolvedValue({
    action: clickAction,
    records: [click, clickAction],
  });
  vi.mocked(recorderApi.videoTimeline).mockResolvedValue(overview);
  vi.mocked(recorderApi.videoFrame).mockImplementation(
    async (_directory, qpc) =>
      ({
        image: { hash: "a", path: "a", bytes: 1, size: [2560, 1600] },
        url: `data:image/png;base64,${qpc}`,
        at_qpc: qpc,
        presented_qpc: qpc,
        acquired_qpc: qpc,
        repeated: false,
        segment: "000001",
        sequence: 1,
        screen_origin: [-2560, 0],
        dpi: [144, 144],
        qpc_frequency: overview.frequency,
      }) satisfies VideoFrame,
  );
  render(
    <PlaybackWorkspace
      directory="workspace"
      action={clickAction}
      records={[clickAction, click]}
      frequency={overview.frequency}
    />,
  );
  await screen.findByTestId("recorded-click");
  fireEvent.click(screen.getByRole("button", { name: "播放回看" }));
  await waitFor(() =>
    expect(
      Number(
        (screen.getByRole("slider", { name: "录制时间轴" }) as HTMLInputElement)
          .value,
      ),
    ).toBeGreaterThan(250),
  );
  fireEvent.click(screen.getByRole("button", { name: "暂停回看" }));
  fireEvent.change(await screen.findByRole("slider", { name: "录制时间轴" }), {
    target: { value: "1500" },
  });
  await waitFor(() =>
    expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
      "workspace",
      timeQpc(overview, 1500),
      0,
      null,
      null,
    ),
  );
  expect(
    await screen.findByRole("heading", { name: "操作对象" }),
  ).toBeInTheDocument();
  expect(screen.queryByTestId("recorded-click")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "记录详情" }),
  ).not.toBeInTheDocument();

  // 同时刻两条轨道仍按点击的原始事件选择；解码帧时间不能移动游标。
  vi.mocked(recorderApi.context).mockRejectedValue(new Error("控件信息不可用"));
  fireEvent.click(screen.getByRole("button", { name: "00:01.000 · 按键 N" }));
  await waitFor(() =>
    expect(recorderApi.context).toHaveBeenLastCalledWith("workspace", 1),
  );
  await waitFor(() =>
    expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
      "workspace",
      timeQpc(overview, 1000),
      -1,
      null,
      null,
    ),
  );
  expect(screen.getByRole("slider", { name: "录制时间轴" })).toHaveValue(
    "1000",
  );
  await screen.findByRole("alert");
  // 操作详情失败也能继续查看首个输入之前的视频。
  fireEvent.change(screen.getByRole("slider", { name: "录制时间轴" }), {
    target: { value: "200" },
  });
  await waitFor(() =>
    expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
      "workspace",
      timeQpc(overview, 200),
      0,
      null,
      null,
    ),
  );
  expect(screen.queryByText("控件信息不可用")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "播放回看" }));
  await waitFor(() =>
    expect(
      Number(
        (screen.getByRole("slider", { name: "录制时间轴" }) as HTMLInputElement)
          .value,
      ),
    ).toBeGreaterThan(600),
  );
  fireEvent.click(screen.getByRole("button", { name: "暂停回看" }));
});
