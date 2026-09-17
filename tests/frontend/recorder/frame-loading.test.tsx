import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useFrameImage } from "../../../src/features/recorder/useFrameImage";
import { recorderApi } from "../../../src/features/recorder/api";
import { visual } from "./review-fixtures";
vi.mock("../../../src/features/recorder/api", () => ({
  recorderApi: { image: vi.fn() },
}));
describe("帧读取异步边界", () => {
  it("切换目录拒绝迟到图片", async () => {
    let finish: (url: string) => void = () => {};
    vi.mocked(recorderApi.image)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      )
      .mockResolvedValueOnce("new");
    const attachment = visual("Input").image;
    const hook = renderHook(
      ({ directory }) => useFrameImage(directory, attachment),
      { initialProps: { directory: "old" } },
    );
    hook.rerender({ directory: "new" });
    expect(hook.result.current.url).toBe("");
    await waitFor(() => expect(hook.result.current.url).toBe("new"));
    await act(async () => {
      finish("old");
    });
    expect(hook.result.current.url).toBe("new");
  });
});
