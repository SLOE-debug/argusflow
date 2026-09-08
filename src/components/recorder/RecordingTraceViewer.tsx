import type { CompletedRecording } from '../../features/recorder';
import { RecordingPrivacyDialog } from './RecordingPrivacyDialog';

/** 录制打开后直接回看，隐私操作与画面共享同一个工作区。 */
export function RecordingTraceViewer({ recording, onSaved }: Readonly<{
  recording: CompletedRecording;
  onSaved: (recording: CompletedRecording) => void;
}>) {
  return <RecordingPrivacyDialog recording={recording} onSaved={onSaved} />;
}
