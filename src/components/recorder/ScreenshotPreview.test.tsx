import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ScreenshotPreview } from './ScreenshotPreview';
import type { ScreenshotEvidence } from '../../features/recorder';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));
/** 同一事件的窗口图像与局部裁切，没有 UIA/CDP 元素。 */
const screenshot: ScreenshotEvidence = {
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

it('loads both PNG artifacts by recording/event identity and releases object URLs', async () => {
  const { unmount } = render(<ScreenshotPreview recordingId="recording" sequence={1} screenshot={screenshot} />);
  await screen.findByAltText('事件发生时的可见窗口区域');
  await screen.findByAltText('鼠标按下位置附近的局部截图');
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
