import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockUiEnvironment } from "../support/ui";
import { RecorderBar } from "../../../src/components/recorder";
import { TitleBar } from "../../../src/components/shell/TitleBar";
import {
  useRecorder,
  type RecorderStatus,
} from "../../../src/features/recorder";

vi.mock("../../../src/features/recorder", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../src/features/recorder")>()),
  useRecorder: vi.fn(),
}));

/** 仅模拟状态与命令，交互测试不启动真实桌面采集。 */
function controller() {
  return {
    status: null,
    entries: [],
    opened: null,
    records: [],
    error: null,
    busy: false,
    end: false,
    following: true,
    setFollowing: vi.fn(),
    start: vi.fn(async () => {}),
    transition: vi.fn(async () => {}),
    safely: vi.fn(async () => {}),
    refresh: vi.fn(async () => {}),
    open: vi.fn(async () => {}),
    loadMore: vi.fn(async () => {}),
  } satisfies ReturnType<typeof useRecorder>;
}

/** 状态由后端提供经过时间，包含暂停时长。 */
const recording: RecorderStatus = {
  session: "test-session",
  phase: "Recording",
  elapsed_ms: 62000,
  operations: 3,
  raw_count: 6,
  pending: 1,
  written: 6,
  synced: 5,
  input_fault: null,
  evidence_gaps: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
  mockUiEnvironment();
});

describe("标题栏录制菜单", () => {
  it("入口位于标题栏，展开与关闭不启动录制，只有选择开始才发送命令", () => {
    const recorder = controller();
    vi.mocked(useRecorder).mockReturnValue(recorder);
    render(<TitleBar menu={<RecorderBar />} tabs={<span>工作流标签</span>} />);
    const trigger = within(screen.getByRole("banner")).getByRole("button", {
      name: "录制菜单",
    });
    expect(screen.queryByText("尚未录制")).not.toBeInTheDocument();
    fireEvent.click(trigger, { detail: 1 });
    expect(screen.getByRole("menu")).toHaveFocus();
    expect(
      screen.getByRole("menuitem", { name: "开始录制" }),
    ).not.toHaveFocus();
    expect(recorder.start).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(trigger).toHaveFocus();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    fireEvent.click(trigger);
    expect(screen.getByRole("menuitem", { name: "开始录制" })).toHaveFocus();
    fireEvent.click(screen.getByRole("menuitem", { name: "开始录制" }));
    expect(recorder.start).toHaveBeenCalledWith();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("录制操作集中在单一菜单，保存中禁止重复停止", () => {
    const recorder = { ...controller(), status: recording };
    vi.mocked(useRecorder).mockReturnValue(recorder);
    const view = render(<RecorderBar />);
    expect(screen.getByText("01:02")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "停止并保存" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "录制菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "停止并保存" }));
    expect(recorder.transition).toHaveBeenCalledWith("Stopped");
    fireEvent.click(screen.getByRole("button", { name: "录制菜单" }));
    expect(
      screen.queryByRole("menuitem", { name: "开始录制" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "暂停录制" }));
    expect(recorder.transition).toHaveBeenCalledWith("Paused");
    vi.mocked(useRecorder).mockReturnValue({
      ...recorder,
      status: { ...recording, phase: "Paused" },
    });
    view.rerender(<RecorderBar />);
    fireEvent.click(screen.getByRole("button", { name: "录制菜单" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "继续录制" }));
    expect(recorder.transition).toHaveBeenCalledWith("Recording");
    vi.mocked(useRecorder).mockReturnValue({
      ...recorder,
      status: { ...recording, phase: "Stopping" },
    });
    view.rerender(<RecorderBar />);
    fireEvent.click(screen.getByRole("button", { name: "录制菜单" }));
    expect(screen.getByRole("menuitem", { name: "停止并保存" })).toBeDisabled();
  });

  it("异常在标题栏可发现，模态回看保留输入错误并移除多余功能", async () => {
    const recorder = {
      ...controller(),
      status: { ...recording, input_fault: "输入连接中断", evidence_gaps: 2 },
    };
    vi.mocked(useRecorder).mockReturnValue(recorder);
    render(<TitleBar menu={<RecorderBar />} tabs={null} />);
    const trigger = screen.getByRole("button", { name: "录制菜单" });
    expect(trigger).toHaveTextContent("录制异常");
    expect(screen.queryByText("正在录制")).not.toBeInTheDocument();
    expect(within(trigger.parentElement!).getAllByRole("button")).toHaveLength(
      1,
    );
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("menuitem", { name: "查看录制" }));
    const panel = await screen.findByRole("dialog", { name: "录制回看" });
    expect(screen.getByRole("banner")).not.toContainElement(panel);
    expect(within(panel).getByRole("alert")).toHaveTextContent("输入连接中断");
    for (const name of [
      "导出",
      "打开数据包",
      "录制设置",
      "上一帧",
      "下一帧",
      "重新读取",
      "识别当前帧",
    ]) {
      expect(
        within(panel).queryByRole("button", { name }),
      ).not.toBeInTheDocument();
    }
    expect(within(panel).queryByText("尚未录制")).not.toBeInTheDocument();
    expect(within(panel).queryByText("操作步骤")).not.toBeInTheDocument();
    fireEvent.click(within(panel).getByRole("button", { name: "关闭" }));
    fireEvent.click(screen.getByRole("button", { name: "录制菜单" }));
    expect(screen.getByRole("menuitem", { name: "停止并保存" })).toBeEnabled();
    fireEvent.click(screen.getByRole("menuitem", { name: "查看录制" }));
    expect(
      screen.getByRole("dialog", { name: "录制回看" }),
    ).toBeInTheDocument();
  });
  it("只有证据缺口时显示单一警告，不宣称输入录制故障", () => {
    vi.mocked(useRecorder).mockReturnValue({
      ...controller(),
      status: { ...recording, phase: "Paused", evidence_gaps: 11 },
    });
    render(<RecorderBar />);
    const trigger = screen.getByRole("button", { name: "录制菜单" });
    expect(trigger).toHaveTextContent("证据不完整");
    expect(trigger).toHaveAttribute(
      "title",
      expect.stringContaining("已暂停 · 11 条证据缺失记录"),
    );
    expect(screen.queryByText("录制异常")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });
});
