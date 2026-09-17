import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { recorderApi } from "../../../src/features/recorder/api";
import { useRecorder } from "../../../src/features/recorder/controller";
import type {
  JournalPage,
  Session,
} from "../../../src/features/recorder/model";
import { action } from "./review-fixtures";

vi.mock("../../../src/features/recorder/api", () => ({
  recorderApi: { list: vi.fn(), status: vi.fn(), open: vi.fn(), read: vi.fn() },
}));
const session: Session = {
  id: "session",
  format: 2,
  created_ms: 0,
  qpc_origin: 0,
  qpc_frequency: 1000,
  capture_session: null,
  policy: "test",
};
function page(sequence: number, end = false): JournalPage {
  return {
    records: [{ ...action, id: sequence }],
    cursor: { offset: sequence * 100, sequence },
    end,
    tail: null,
  };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(recorderApi.list).mockResolvedValue([]);
  vi.mocked(recorderApi.status).mockResolvedValue({
    session: null,
    phase: null,
    elapsed_ms: 0,
    operations: 0,
    raw_count: 0,
    pending: 0,
    written: 0,
    synced: 0,
    input_fault: null,
    evidence_gaps: 0,
  });
  vi.mocked(recorderApi.open).mockResolvedValue(session);
});
describe("回看分页边界", () => {
  it("一次最多读取8页，继续加载从最后游标前进", async () => {
    vi.mocked(recorderApi.read).mockImplementation(async (_, cursor) =>
      page(cursor.sequence + 1),
    );
    const hook = renderHook(() => useRecorder());
    await act(async () => {
      await hook.result.current.open("recording");
    });
    await waitFor(() => expect(hook.result.current.records).toHaveLength(8));
    expect(recorderApi.read).toHaveBeenCalledTimes(8);
    await act(async () => {
      await hook.result.current.loadMore();
    });
    expect(hook.result.current.records).toHaveLength(16);
    expect(recorderApi.read).toHaveBeenNthCalledWith(9, "recording", {
      offset: 800,
      sequence: 8,
    });
  });
  it("切换目录后迟到的页不能混入新录制，也不能继续读取旧目录", async () => {
    let finish: (value: JournalPage) => void = () => {};
    vi.mocked(recorderApi.read).mockImplementation(async (directory) =>
      directory === "old"
        ? new Promise<JournalPage>((resolve) => {
            finish = resolve;
          })
        : page(20, true),
    );
    const hook = renderHook(() => useRecorder());
    await act(async () => {
      await hook.result.current.open("old");
    });
    await waitFor(() => expect(recorderApi.read).toHaveBeenCalledTimes(1));
    await act(async () => {
      await hook.result.current.open("new");
    });
    await waitFor(() => expect(hook.result.current.end).toBe(true));
    await act(async () => {
      finish(page(1));
    });
    expect(hook.result.current.records.map((record) => record.id)).toEqual([
      20,
    ]);
    expect(recorderApi.read).toHaveBeenCalledTimes(2);
    expect(hook.result.current.opened?.directory).toBe("new");
  });
});
