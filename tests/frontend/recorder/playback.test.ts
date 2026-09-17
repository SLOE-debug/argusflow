import { describe, expect, it } from "vitest";
import {
  qpcTime,
  timeQpc,
  timelineMarkers,
} from "../../../src/features/recorder/playback";
import { overview } from "./playback-fixtures";
describe("回看时钟与标记", () => {
  it("超过JS安全整数的绝对QPC仍能精确定位毫秒", () => {
    expect(timeQpc(overview, 1234.5)).toBe("9007199267086000");
    expect(qpcTime(overview, timeQpc(overview, 1234.5))).toBe(1234.5);
  });
  it("同一时刻键鼠分别保留，高密度标记不会无限扩张DOM", () => {
    expect(timelineMarkers(overview).map((m) => m.lane)).toEqual([0, 1]);
    const markers = Array.from({ length: 100000 }, (_, id) => ({
      id,
      time_ms: id / 10,
      input: { kind: "key" as const, vk: 78 },
    }));
    expect(
      timelineMarkers({ ...overview, markers }).length,
    ).toBeLessThanOrEqual(1201);
  });
});
