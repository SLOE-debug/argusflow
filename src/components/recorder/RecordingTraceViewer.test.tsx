import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { RECORDING_FIXTURE } from '../../features/recorder/testFixtures';
import { RecordingTraceViewer } from './RecordingTraceViewer';

it('opens playback directly and shows only the current event summary', () => {
  render(<RecordingTraceViewer recording={RECORDING_FIXTURE} onSaved={vi.fn()} />);
  expect(screen.getByRole('slider', { name: '录制时间轴' })).toBeInTheDocument();
  expect(screen.queryByText('时间线 JSON')).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole('slider', { name: '录制时间轴' }), { target: { value: '10' } });
  expect(screen.getByText(/键盘按下/)).toBeInTheDocument();
  expect(screen.getByText(/测试窗口/)).toBeInTheDocument();
  expect(screen.getByText('UIA')).toBeInTheDocument();
  expect(screen.getByText('AutomationId')).toBeInTheDocument();
  expect(screen.getByText('password')).toBeInTheDocument();
  expect(screen.queryByText('OCR：未记录')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '下一操作' })).not.toBeInTheDocument();
});

it('provides editable endpoints and keyboard-operable range handles', () => {
  const source = RECORDING_FIXTURE.trace.timeline.events[0];
  const recording = { ...RECORDING_FIXTURE, trace: { ...RECORDING_FIXTURE.trace, timeline: { events: [source, { ...source, sequence: 2, elapsed_ms: 10000 }] } } };
  render(<RecordingTraceViewer recording={recording} onSaved={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: '选择时间段' }));
  expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '调整时间' }));
  expect(screen.getByRole('spinbutton', { name: '片段终点（秒）' })).toHaveValue(2);
  fireEvent.change(screen.getByRole('spinbutton', { name: '片段终点（秒）' }), { target: { value: '4.5' } });
  expect(screen.getByRole('slider', { name: '片段终点' })).toHaveAttribute('aria-valuenow', '4500');
  fireEvent.keyDown(screen.getByRole('slider', { name: '片段起点' }), { key: 'ArrowRight' });
  expect(screen.getByRole('spinbutton', { name: '片段起点（秒）' })).toHaveValue(0.001);
  fireEvent.click(screen.getByRole('button', { name: '取消选择片段 1' }));
  expect(screen.queryByRole('slider', { name: '片段起点' })).not.toBeInTheDocument();
});

it('preserves the selected interval while browsing and does not duplicate it when switching modes', () => {
  render(<RecordingTraceViewer recording={RECORDING_FIXTURE} onSaved={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: '选择时间段' }));
  expect(screen.getByRole('combobox', { name: '选择要处理的内容' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '浏览画面' }));
  expect(screen.getByRole('button', { name: '浏览画面' })).toHaveAttribute('aria-pressed', 'true');
  expect(screen.getByRole('slider', { name: '片段起点' })).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: '选择要处理的内容' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '选择时间段' }));
  expect(screen.getAllByRole('button', { name: /取消选择片段/ })).toHaveLength(1);
});
