import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { RECORDING_FIXTURE } from '../../features/recorder/testFixtures';
import { RecordingPrivacyDialog } from './RecordingPrivacyDialog';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));
beforeEach(() => invoke.mockReset());

it('selects a timeline without OCR and confirms actual records before deletion', async () => {
  const onSaved = vi.fn();
  const updated = { ...RECORDING_FIXTURE, trace: { ...RECORDING_FIXTURE.trace, timeline: { events: [] } } };
  invoke.mockResolvedValue(updated);
  render(<RecordingPrivacyDialog recording={RECORDING_FIXTURE} onSaved={onSaved} onClose={vi.fn()} />);
  expect(screen.getByRole('slider', { name: '录制时间轴' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '选择时间段' }));
  fireEvent.click(screen.getByRole('combobox', { name: '选择要处理的内容' }));
  fireEvent.click(screen.getByRole('option', { name: /删除操作/ }));
  fireEvent.click(screen.getByRole('button', { name: '处理所选内容' }));
  expect(invoke).not.toHaveBeenCalled();
  expect(screen.getByText('删除 1 条操作记录？')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '删除记录' }));
  await waitFor(() => expect(onSaved).toHaveBeenCalledWith(updated));
  expect(invoke).toHaveBeenCalledWith('edit_recording_privacy', {
    recordingId: RECORDING_FIXTURE.trace.recording_id,
    edits: [{ type: 'erase_event', sequence: 1 }],
  });
});
