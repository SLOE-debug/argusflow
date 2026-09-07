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
    await screen.findByRole('button', { name: '录制中' });
    fireEvent.click(screen.getByRole('button', { name: '收起录制面板' }));
    expect(api.stopRecording).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '录制中' }));
    fireEvent.click(screen.getByRole('button', { name: '停止并保存' }));
    await screen.findByText('步骤 1 · 定位详情');
    expect(screen.getByText('AutomationId', { selector: 'dt' })).toBeInTheDocument();
    expect(screen.getByText('首选')).toBeInTheDocument();
    expect(screen.getByText('定位候选尚未验证唯一性')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '原始 Trace' }));
    expect(screen.getByLabelText('原始 Trace').textContent).toContain('"virtual_key": null');
  });

  it('copies only the chosen sanitized trace layer and displays clipboard failures', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    fireEvent.click(await screen.findByRole('button', { name: /1 个步骤 · 1 个事件/ }));
    await screen.findByText('步骤 1 · 定位详情');
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '复制语义 Trace' })); });
    expect(JSON.parse(writeText.mock.calls[0][0])).toEqual(RECORDING_FIXTURE.trace.normalized);
    writeText.mockRejectedValueOnce(new Error('denied'));
    fireEvent.click(screen.getByRole('button', { name: '原始 Trace' }));
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '复制原始 Trace' })); });
    expect(screen.getByText('复制失败，请使用导出 JSON。')).toBeInTheDocument();
  });

  it('explains unsupported IME commits even when no semantic step was created', async () => {
    api.getRecording.mockResolvedValue({ ...RECORDING_FIXTURE, trace: { ...RECORDING_FIXTURE.trace,
      normalized: { records: [], diagnostics: [{ type: 'keyboard_decode', reason: 'input_method_active' }] },
    } });
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '录制操作' }));
    fireEvent.click(await screen.findByRole('button', { name: /1 个步骤 · 1 个事件/ }));
    expect(await screen.findByText('暂不支持输入法组合提交，组合期间的文字未录入')).toBeInTheDocument();
    expect(screen.getByText('没有可归一化的操作。可查看原始 Trace 和诊断。')).toBeInTheDocument();
  });
});
