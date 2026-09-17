import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, it, expect, vi } from "vitest";
import { EvidenceDetails } from "../../../src/components/recorder/EvidenceDetails";
import {
  action as keyboardAction,
  clickAction as action,
  click,
} from "./review-fixtures";
import { recorderApi } from "../../../src/features/recorder/api";
import type { VideoFrame } from "../../../src/features/recorder/video";
vi.mock("../../../src/features/recorder/api", () => ({
  recorderApi: { videoFrame: vi.fn() },
}));
const frame: VideoFrame = {
  image: { hash: "a", path: "attachments/a.png", size: [100, 100], bytes: 10 },
  url: "data:image/png;base64,frame",
  at_qpc: "100",
  presented_qpc: "100",
  acquired_qpc: "110",
  segment: "000001",
  sequence: 1,
  repeated: false,
  screen_origin: [0, 0],
  dpi: [96, 96],
  qpc_frequency: 1000,
};
beforeEach(() => {
  vi.mocked(recorderApi.videoFrame).mockReset();
});
describe("视频事件回看", () => {
  it.each(["SystemKey", "TextUnconfirmed", "Drag", "Scroll"] as const)(
    "按操作类型选择默认画面：%s",
    async (kind) => {
      if (!("Interaction" in keyboardAction.data)) throw Error("fixture");
      const selected = {
        ...keyboardAction,
        data: { Interaction: { ...keyboardAction.data.Interaction, kind } },
      };
      vi.mocked(recorderApi.videoFrame).mockResolvedValue(frame);
      render(
        <EvidenceDetails
          frequency={1000}
          directory={kind}
          action={selected}
          records={[selected]}
        />,
      );
      await screen.findByRole("img");
      const mouse = kind === "Drag" || kind === "Scroll";
      expect(
        screen.getByRole("button", { name: mouse ? "操作时刻" : "操作结果" }),
      ).toHaveAttribute("aria-pressed", "true");
      expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
        kind,
        mouse ? "100" : "420",
        mouse ? -1 : 0,
        mouse ? null : "100",
        null,
      );
    },
  );

  it("StrictMode重复挂载共用取帧请求，不能停留在忙碌错误", async () => {
    let resolve!: (frame: VideoFrame) => void;
    let active = false;
    vi.mocked(recorderApi.videoFrame).mockImplementation(() => {
      if (active)
        return Promise.reject(Error("正在读取其他视频帧，请稍后重试"));
      active = true;
      return new Promise((r) => {
        resolve = (value) => {
          active = false;
          r(value);
        };
      });
    });
    render(
      <StrictMode>
        <EvidenceDetails
          frequency={1000}
          directory="strict"
          action={action}
          records={[action]}
        />
      </StrictMode>,
    );
    await act(async () => resolve(frame));
    expect(await screen.findByRole("img")).toHaveAttribute("src", frame.url);
    expect(recorderApi.videoFrame).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("直接取帧，只保留操作时刻与结果切换", async () => {
    vi.mocked(recorderApi.videoFrame).mockResolvedValue(frame);
    render(
      <EvidenceDetails
        frequency={1000}
        directory="session"
        action={action}
        records={[action]}
      />,
    );
    expect(
      await screen.findByRole("img", { name: "按操作时间读取的录制画面" }),
    ).toHaveAttribute("src", frame.url);
    expect(
      screen.queryByRole("button", { name: "下一帧" }),
    ).not.toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(2);
    expect(screen.getAllByRole("img")).toHaveLength(1);
  });
  it("缺失的视频明确显示错误，不拿旧截图填补", async () => {
    vi.mocked(recorderApi.videoFrame).mockRejectedValue(
      Error("此时刻没有已保存的视频帧"),
    );
    render(
      <EvidenceDetails
        frequency={1000}
        directory="session"
        action={action}
        records={[action]}
      />,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "没有已保存的视频帧",
    );
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "操作结果" })).toBeEnabled();
  });
  it("切换录制后忽略旧解码请求的迟到结果", async () => {
    let resolve!: (frame: VideoFrame) => void;
    vi.mocked(recorderApi.videoFrame)
      .mockImplementationOnce(
        () =>
          new Promise((r) => {
            resolve = r;
          }),
      )
      .mockResolvedValue({ ...frame, url: "data:image/png;base64,new" });
    const view = render(
      <EvidenceDetails
        frequency={1000}
        directory="old"
        action={action}
        records={[action]}
      />,
    );
    view.rerender(
      <EvidenceDetails
        frequency={1000}
        directory="new"
        action={action}
        records={[action]}
      />,
    );
    expect(recorderApi.videoFrame).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    await act(async () => resolve(frame));
    expect(await screen.findByRole("img")).toHaveAttribute(
      "src",
      "data:image/png;base64,new",
    );
    expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
      "new",
      "100",
      -1,
      null,
      null,
    );
  });
  it("点击默认取按下前画面并标记物理坐标，结果视图不能带目标标记", async () => {
    vi.mocked(recorderApi.videoFrame).mockResolvedValue({
      ...frame,
      image: { ...frame.image, size: [2560, 1600] },
      screen_origin: [-2560, 0],
      dpi: [144, 144],
    });
    render(
      <EvidenceDetails
        frequency={1000}
        directory="target"
        action={action}
        records={[action, click]}
      />,
    );
    const marker = await screen.findByTestId("recorded-click");
    expect(marker).toHaveAttribute("cx", "1280");
    expect(marker).toHaveAttribute("cy", "800");
    expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
      "target",
      "100",
      -1,
      null,
      null,
    );
    expect(screen.getByRole("button", { name: "操作时刻" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "操作结果" }));
    await waitFor(() =>
      expect(recorderApi.videoFrame).toHaveBeenLastCalledWith(
        "target",
        "2120",
        0,
        "100",
        "420",
      ),
    );
    await screen.findByRole("img");
    expect(screen.queryByTestId("recorded-click")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "操作时刻" }));
    expect(await screen.findByTestId("recorded-click")).toHaveAttribute(
      "cx",
      "1280",
    );
  });
});
