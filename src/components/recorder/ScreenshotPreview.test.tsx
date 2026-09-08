import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ScreenshotPreview } from './ScreenshotPreview';
import type { ScreenshotEvidence } from '../../features/recorder';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));
/** 同一事件的窗口图像与局部裁切，没有 UIA/CDP 元素。 */
const screenshot: ScreenshotEvidence = {
  stabilized: true, click_color: [0, 255, 255],
  path: 'evidence/1.png', captured_at_ms: 10, capture_duration_ms: 2,
  screen_bounds: { x: -100, y: 0, width: 800, height: 600 }, width: 800, height: 600,
  pointer: { x: -80, y: 20 }, crop: { path: 'evidence/1-crop.png', bounds: { x: 0, y: 0, width: 116, height: 116 } },
  crop_failure: null,
};

beforeEach(() => {
  vi.stubGlobal('URL', { createObjectURL: vi.fn().mockReturnValue('blob:evidence'), revokeObjectURL: vi.fn() });
  invoke.mockReset().mockResolvedValue(new ArrayBuffer(8));
});
afterEach(() => vi.unstubAllGlobals());

it('reads click target artifacts separately from the later result', async () => {
  render(<ScreenshotPreview recordingId="recording" sequence={1} screenshot={screenshot} target />);
  await screen.findByAltText('点击时的目标窗口区域');
  await screen.findByAltText('鼠标按下位置附近的局部截图');
  expect(invoke).toHaveBeenCalledWith('read_recording_screenshot', { recordingId: 'recording', sequence: 1, kind: 'target' });
  expect(invoke).toHaveBeenCalledWith('read_recording_screenshot', { recordingId: 'recording', sequence: 1, kind: 'target_crop' });
  expect(screen.queryByText(/画面已稳定/)).not.toBeInTheDocument();
});

it('loads both PNG artifacts by recording/event identity and releases object URLs', async () => {
  const { unmount } = render(<ScreenshotPreview recordingId="recording" sequence={1} screenshot={screenshot} />);
  await screen.findByAltText('操作后的可见窗口区域');
  await screen.findByAltText('鼠标按下位置附近的局部截图');
  const marker = screen.getByRole('img', { name: '点击位置框' }).querySelector('rect');
  expect(marker).toHaveAttribute('x', '4');
  expect(marker).toHaveAttribute('y', '4');
  expect(marker).toHaveAttribute('stroke', 'rgb(0,255,255)');
  expect(screen.getByText('点击局部放大').parentElement).toHaveClass('absolute');
  expect(invoke).toHaveBeenCalledWith('read_recording_screenshot', { recordingId: 'recording', sequence: 1, kind: 'window' });
  expect(invoke).toHaveBeenCalledWith('read_recording_screenshot', { recordingId: 'recording', sequence: 1, kind: 'crop' });
  unmount();
  expect(URL.revokeObjectURL).toHaveBeenCalledTimes(2);
});

it('does not create an image URL for a request completed after unmount', async () => {
  let finish: (bytes: ArrayBuffer) => void = () => undefined;
  invoke.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const { unmount } = render(<ScreenshotPreview recordingId="recording" sequence={1} screenshot={{ ...screenshot, crop: null }} />);
  unmount();
  await act(async () => { finish(new ArrayBuffer(8)); });
  expect(URL.createObjectURL).not.toHaveBeenCalled();
});

it('clips edge markers, keeps the inset away from the click and reports unsettled capture', async () => {
  render(<ScreenshotPreview
    recordingId="recording"
    sequence={1}
    screenshot={{ ...screenshot, pointer: { x: 699, y: 599 }, stabilized: false }}
  />);
  await screen.findByAltText('操作后的可见窗口区域');
  const marker = screen.getByRole('img', { name: '点击位置框' }).querySelector('rect');
  expect(marker).toHaveAttribute('x', '783');
  expect(marker).toHaveAttribute('width', '17');
  expect(marker).toHaveAttribute('height', '17');
  expect(screen.getByText('点击局部放大').parentElement).toHaveClass('left-2');
  expect(screen.getByText(/画面尚未确认稳定/)).toBeInTheDocument();
});
