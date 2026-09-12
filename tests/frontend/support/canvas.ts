import { vi } from "vitest";

/** 记录绘制调用；不冒充真实像素或浏览器视觉验收。 */
export function recordingContext() {
  const gradient = { addColorStop: vi.fn() };
  const calls = {
    createLinearGradient: vi.fn(() => gradient),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    strokeRect: vi.fn(),
    setTransform: vi.fn(),
    translate: vi.fn(),
    scale: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    beginPath: vi.fn(),
    closePath: vi.fn(),
    roundRect: vi.fn(),
    arc: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    setLineDash: vi.fn(),
    fillText: vi.fn(),
    drawImage: vi.fn(),
    measureText: vi.fn((value: string) => ({
      width: Array.from(value).length * 7,
    })),
    font: "",
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    globalAlpha: 1,
    textBaseline: "alphabetic",
    textAlign: "left",
    shadowBlur: 0,
    shadowColor: "",
    shadowOffsetY: 0,
  };
  return {
    calls,
    gradient,
    context: calls as unknown as CanvasRenderingContext2D,
  };
}

/** 每个测试显式安装绘制替身，由 restoreAllMocks 清理。 */
export function mockCanvas() {
  const recording = recordingContext();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    recording.context,
  );
  return recording;
}
