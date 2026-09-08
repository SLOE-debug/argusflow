import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useRecorder, IDLE_RECORDER_STATUS } from '../../features/recorder';
import { RECORDING_FIXTURE, RECORDING_STATUS, RECORDING_SUMMARY } from '../../features/recorder/testFixtures';
import { RecorderToolbar } from './RecorderToolbar';

const api = vi.hoisted(() => ({ recorderAvailable: vi.fn(), startRecording: vi.fn(),
  stopRecording: vi.fn(), getRecordingStatus: vi.fn(), listRecordings: vi.fn(), getRecording: vi.fn() }));
vi.mock('../../features/recorder/api', () => api);

function Harness() { return <RecorderToolbar recorder={useRecorder()} workflowRunning={false} />; }

describe('recorder user flow', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.recorderAvailable.mockReturnValue(true);
    api.getRecordingStatus.mockResolvedValue(IDLE_RECORDER_STATUS);
    api.startRecording.mockResolvedValue(RECORDING_STATUS);
    api.stopRecording.mockResolvedValue(RECORDING_FIXTURE);
    api.listRecordings.mockResolvedValue([RECORDING_SUMMARY]);
    api.getRecording.mockResolvedValue(RECORDING_FIXTURE);
  });

  it('starts explicitly, stays recording when collapsed, stops and shows semantic facts', async () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '开始录制' })).toBeEnabled());
    expect(api.startRecording).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '开始录制' }));
    expect(api.startRecording).toHaveBeenCalledWith(expect.objectContaining({ enabled: false }));
    await screen.findByRole('button', { name: '录制中' });
    fireEvent.click(screen.getByRole('button', { name: '收起录制面板' }));
    expect(api.stopRecording).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '录制中' }));
    fireEvent.click(screen.getByRole('button', { name: '停止并保存' }));
    expect(await screen.findByRole('slider', { name: '录制时间轴' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '时间线 JSON' })).not.toBeInTheDocument();
  });

  it('offers privacy review after saving instead of a recording-wide switch', async () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '开始录制' })).toBeEnabled());
    expect(screen.queryByRole('checkbox', { name: '开启敏感数据遮盖' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '开始录制' }));
    expect(api.startRecording).toHaveBeenCalledWith(expect.objectContaining({ enabled: false }));
    await screen.findByRole('button', { name: '录制中' });
    fireEvent.click(screen.getByRole('button', { name: '停止并保存' }));
    expect(await screen.findByRole('button', { name: '选择时间段' })).toBeEnabled();
  });

  it('copies only the chosen sanitized trace layer and displays clipboard failures', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    fireEvent.click(screen.getByRole('button', { name: '录制历史' }));
    fireEvent.click(await screen.findByRole('button', { name: /1 条操作记录 · 0 份截图/ }));
    expect(await screen.findByRole('slider', { name: '录制时间轴' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '复制时间线' })).not.toBeInTheDocument();
  });

  it('explains unsupported IME commits even when no semantic step was created', async () => {
    api.getRecording.mockResolvedValue({ ...RECORDING_FIXTURE, trace: { ...RECORDING_FIXTURE.trace,
      timeline: { events: [{ ...RECORDING_FIXTURE.trace.timeline.events[0], diagnostics: [{ type: 'keyboard_decode', reason: 'input_method_active' }] }] },
    } });
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    fireEvent.click(screen.getByRole('button', { name: '录制历史' }));
    fireEvent.click(await screen.findByRole('button', { name: /1 条操作记录 · 0 份截图/ }));
    fireEvent.change(await screen.findByRole('slider', { name: '录制时间轴' }), { target: { value: '10' } });
    expect(screen.getByText(/暂不支持输入法组合提交，组合期间的文字未录入/)).toBeInTheDocument();
  });
});
