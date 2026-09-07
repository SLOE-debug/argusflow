import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IDLE_RECORDER_STATUS } from './model';
import { RECORDING_FIXTURE, RECORDING_STATUS, RECORDING_SUMMARY } from './testFixtures';

const api = vi.hoisted(() => ({ recorderAvailable: vi.fn(), startRecording: vi.fn(),
  stopRecording: vi.fn(), getRecordingStatus: vi.fn(), listRecordings: vi.fn(), getRecording: vi.fn() }));
vi.mock('./api', () => api);
import { useRecorder } from './useRecorder';

describe('recorder lifecycle', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.recorderAvailable.mockReturnValue(true);
    api.getRecordingStatus.mockResolvedValue(IDLE_RECORDER_STATUS);
    api.startRecording.mockResolvedValue(RECORDING_STATUS);
    api.stopRecording.mockResolvedValue(RECORDING_FIXTURE);
    api.listRecordings.mockResolvedValue([RECORDING_SUMMARY]);
    api.getRecording.mockResolvedValue(RECORDING_FIXTURE);
  });

  it('never installs hooks on mount and restores an existing global session', async () => {
    api.getRecordingStatus.mockResolvedValue(RECORDING_STATUS);
    const { result, unmount } = renderHook(useRecorder);
    await waitFor(() => expect(result.current.status.phase).toBe('recording'));
    expect(api.startRecording).not.toHaveBeenCalled();
    unmount();
    expect(api.stopRecording).not.toHaveBeenCalled();
  });

  it('serializes double start and returns saved trace/history on stop', async () => {
    const { result } = renderHook(useRecorder);
    await waitFor(() => expect(result.current.ready).toBe(true));
    await act(async () => { await Promise.all([result.current.start(), result.current.start()]); });
    expect(api.startRecording).toHaveBeenCalledOnce();
    expect(result.current.status.phase).toBe('recording');
    await act(async () => { await result.current.stop(); });
    expect(result.current.status.phase).toBe('idle');
    expect(result.current.completed).toEqual(RECORDING_FIXTURE);
    expect(result.current.history).toEqual([RECORDING_SUMMARY]);
  });

  it('recovers stopped-but-unsaved state without starting another hook', async () => {
    api.getRecordingStatus.mockResolvedValue({ ...RECORDING_STATUS, phase: 'awaiting_save' });
    api.stopRecording.mockRejectedValueOnce('disk full');
    const { result } = renderHook(useRecorder);
    await waitFor(() => expect(result.current.ready).toBe(true));
    await act(async () => { await result.current.stop(); });
    expect(result.current.error).toContain('disk full');
    expect(result.current.status.phase).toBe('awaiting_save');
    await act(async () => { await result.current.stop(); });
    expect(result.current.completed).toEqual(RECORDING_FIXTURE);
    expect(api.startRecording).not.toHaveBeenCalled();
  });

  it('does not pretend the browser preview can record global input', async () => {
    api.recorderAvailable.mockReturnValue(false);
    const { result } = renderHook(useRecorder);
    await act(async () => { await result.current.start(); });
    expect(result.current.available).toBe(false);
    expect(api.getRecordingStatus).not.toHaveBeenCalled();
    expect(api.startRecording).not.toHaveBeenCalled();
  });

  it('ignores a slow history response after a new recording starts', async () => {
    let resolveHistory: (value: typeof RECORDING_FIXTURE) => void = () => undefined;
    api.getRecording.mockImplementation(() => new Promise((resolve) => { resolveHistory = resolve; }));
    const { result } = renderHook(useRecorder);
    await waitFor(() => expect(result.current.ready).toBe(true));
    let historyRequest: Promise<void>;
    act(() => { historyRequest = result.current.selectHistory(RECORDING_SUMMARY.recording_id); });
    await act(async () => { await result.current.start(); });
    await act(async () => { resolveHistory(RECORDING_FIXTURE); await historyRequest; });
    expect(result.current.completed).toBeNull();
    expect(result.current.status.phase).toBe('recording');
  });
});
