import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { aiApi } from "../../../src/features/ai/api";
import { useAiAnalysis } from "../../../src/features/ai/useAiAnalysis";
import { useAiSettings } from "../../../src/features/ai/useAiSettings";
import type { AiResult, ConfigView } from "../../../src/features/ai/model";
vi.mock("../../../src/features/ai/api", () => ({
  aiApi: { config: vi.fn(), save: vi.fn(), analyze: vi.fn(), cancel: vi.fn() },
}));
const view: ConfigView = {
  config: {
    endpoint: "https://example.test/chat/completions",
    model: "model",
    vision: true,
    bailian: true,
    max_rounds: 4,
    max_tools: 6,
    max_tokens: 16000,
    timeout_seconds: 180,
  },
  has_key: true,
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(aiApi.cancel).mockResolvedValue();
});
it("保存配置后清除输入框密钥，普通更新不回传旧密钥", async () => {
  vi.mocked(aiApi.config).mockResolvedValue(view);
  vi.mocked(aiApi.save).mockResolvedValue(view);
  const { result } = renderHook(() => useAiSettings());
  await waitFor(() => expect(result.current.config).not.toBeNull());
  act(() => result.current.setKey("new-secret"));
  await act(() => result.current.save());
  expect(aiApi.save).toHaveBeenCalledWith(view.config, "new-secret");
  expect(result.current.key).toBe("");
  await act(() => result.current.save());
  expect(aiApi.save).toHaveBeenLastCalledWith(view.config, null);
});
it("更换录制取消原请求，晚到结果不能覆盖新录制", async () => {
  let resolve!: (v: AiResult) => void;
  vi.mocked(aiApi.analyze).mockImplementation(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  const hook = renderHook(({ directory }) => useAiAnalysis(directory), {
    initialProps: { directory: "first" },
  });
  let pending!: Promise<void>;
  act(() => {
    pending = hook.result.current.run();
  });
  const id = vi.mocked(aiApi.analyze).mock.calls[0][0];
  hook.rerender({ directory: "second" });
  expect(aiApi.cancel).toHaveBeenCalledWith(id);
  await act(async () => {
    resolve({
      file: null,
      analysis: {
        summary: "old",
        node_evidence: [],
        unresolved: [],
        required_bindings: [],
        replay_ready: false,
      },
      metrics: { rounds: 1, tool_calls: 0, image_pixels: 0, usage: [] },
    });
    await pending;
  });
  expect(hook.result.current.result).toBeNull();
  expect(hook.result.current.running).toBe(false);
});
it("配置失败后可以重试读取", async () => {
  vi.mocked(aiApi.config)
    .mockRejectedValueOnce(new Error("database busy"))
    .mockResolvedValueOnce(view);
  const hook = renderHook(() => useAiSettings());
  await waitFor(() =>
    expect(hook.result.current.error).toContain("database busy"),
  );
  await act(() => hook.result.current.load());
  expect(hook.result.current.view?.has_key).toBe(true);
  expect(hook.result.current.error).toBeNull();
});
