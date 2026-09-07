import { act, fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { RECORDING_FIXTURE } from '../../features/recorder/testFixtures';
import type { CompletedRecording, PointerMotion } from '../../features/recorder';
import { RecordingTraceViewer } from './RecordingTraceViewer';

/** 同一连续移动的源事件仍可追溯，只有关键点进入时间线 JSON。 */
const motion: PointerMotion = {
  sample_count: 1738, end_sequence: 1738, ended_ms: 1737, distance_px: 1737, pressed_buttons: [],
  points: [
    { sequence: 1, elapsed_ms: 0, point: { x: -2000, y: -100 } },
    { sequence: 1738, elapsed_ms: 1737, point: { x: -263, y: -100 } },
  ],
};
const recording: CompletedRecording = {
  ...RECORDING_FIXTURE,
  trace: { ...RECORDING_FIXTURE.trace, timeline: { events: [
    { sequence: 1, timestamp_ms: 0, elapsed_ms: 0, input: { type: 'pointer_motion', ...motion }, evidence: null, diagnostics: [] },
    { ...RECORDING_FIXTURE.trace.timeline.events[0], sequence: 1739, elapsed_ms: 1800 },
  ] } },
};

it('shows a whole movement as one row without empty evidence warnings or dozens of pages', () => {
  render(<RecordingTraceViewer recording={recording} />);
  expect(screen.getByText('鼠标移动轨迹')).toBeInTheDocument();
  expect(screen.getByText('1738 个采样点 → 2 个轨迹点')).toBeInTheDocument();
  expect(screen.getByText(/来自 1739 个原始采样/)).toBeInTheDocument();
  expect(screen.queryByText('窗口信息缺失')).not.toBeInTheDocument();
  expect(screen.queryByText('未保存截图')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '下一页' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /^2\. 键盘按下/ }));
  expect(screen.getByText('事件 1739 · 证据详情')).toBeInTheDocument();
});

it('copies and displays the same compact timeline with source counts and point coordinates', async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
  render(<RecordingTraceViewer recording={recording} />);
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: '复制时间线' })); });
  expect(JSON.parse(writeText.mock.calls[0][0])).toEqual(recording.trace);
  fireEvent.click(screen.getByRole('button', { name: '时间线 JSON' }));
  const json = JSON.parse(screen.getByLabelText('时间线 JSON').textContent ?? '{}');
  expect(json.timeline.events).toHaveLength(2);
  expect(json.timeline.events[0].input).toMatchObject({ type: 'pointer_motion', sample_count: 1738, end_sequence: 1738 });
});
