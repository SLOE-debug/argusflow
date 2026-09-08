import { useMemo, useRef, useState } from 'react';
import { preciseDurationLabel, editRecordingPrivacy, timelinePrivacyEdits, usePrivacyPlayback, type CompletedRecording, type PrivacyEdit, type PrivacyTimeRange, type PrivacyTreatment } from '../../features/recorder';
import { Button, ConfirmDialog, Select } from '../ui';
import { PrivacyScreenshotEditor } from './PrivacyScreenshotEditor';
import { PrivacyPlaybackOverlay } from './PrivacyPlaybackOverlay';
import { PrivacyTimeline } from './PrivacyTimeline';

/** 回看、片段选择与处理确认的装配入口，无需先扫描截图。 */
export function RecordingPrivacyDialog({ recording, onSaved }: Readonly<{
  recording: CompletedRecording;
  onSaved: (recording: CompletedRecording) => void;
  onClose?: () => void;
}>) {
  const events = recording.trace.timeline.events;
  const playback = usePrivacyPlayback(events);
  const [ranges, setRanges] = useState<readonly PrivacyTimeRange[]>([]);
  const [treatment, setTreatment] = useState<PrivacyTreatment>('screenshots');
  const [confirmation, setConfirmation] = useState<readonly PrivacyEdit[]>([]);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [revision, setRevision] = useState(0);
  const [notice, setNotice] = useState('先回看，再选择需要处理的片段。无需等待文字识别。');
  /** 阻止确认按钮在状态提交前重复发出不可逆写入。 */
  const locked = useRef(false);
  const edits = useMemo(() => timelinePrivacyEdits(events, playback.frames, ranges, treatment, playback.duration), [events, playback.frames, ranges, treatment, playback.duration]);
  const save = async () => {
    if (locked.current || !confirmation.length) return;
    locked.current = true;
    setSaving(true);
    try {
      const updated = await editRecordingPrivacy(recording.trace.recording_id, confirmation);
      onSaved(updated);
      setRevision((value) => value + 1);
      setRanges([]);
      setEditing(false);
      setNotice('已保存隐私处理。你可以继续回看检查。');
    } catch {
      setNotice('保存未完成，部分内容可能已经处理。请关闭并重新打开这份录制，检查后再试。');
    } finally {
      setConfirmation([]);
      locked.current = false;
      setSaving(false);
    }
  };
  const frame = playback.frame;
  const removing = confirmation.some((edit) => edit.type === 'erase_event');
  return (
    <section aria-label="录制回看" className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <p role="status" className="mr-auto text-xs text-slate-600">{notice}</p>
          <Button
            disabled={saving || !frame}
            variant={editing ? 'primary' : 'secondary'}
            onClick={() => { playback.pause(); setEditing((value) => !value); }}
          >{editing ? '完成框选' : '遮盖画面中的一块'}</Button>
        </div>
        <div className="min-h-0 flex-1 overflow-hidden rounded-lg bg-slate-100 p-1">
          {frame ? (
            <div className="mx-auto flex h-full min-h-0 w-full flex-col">
              <PrivacyScreenshotEditor
                key={`${frame.sequence}:${frame.kind}:${revision}:${editing}`}
                recordingId={recording.trace.recording_id}
                sequence={frame.sequence}
                kind={frame.kind}
                screenshot={frame.shot}
                highlights={[]}
                disabled={saving}
                editing={editing}
                overlay={editing ? undefined : <PrivacyPlaybackOverlay events={events} time={playback.time} frame={frame} />}
                onMosaic={(rect) => setConfirmation([{ type: 'mosaic', sequence: frame.sequence, kind: frame.kind, rect }])}
              />
              <p className="mt-2 text-center text-xs text-slate-500">屏幕采样于 {preciseDurationLabel(frame.shot.captured_at_ms)} · 下一张采样出现前保留此画面</p>
            </div>
          ) : (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-sm text-slate-500">
              <p>{events.length ? '这个时间点还没有保存的屏幕采样。' : '这份录制已没有操作记录。'}</p>
              {playback.frames[0] ? <Button onClick={() => playback.seek(playback.frames[0].shot.captured_at_ms)}>查看第一张画面</Button> : <p>仍可在时间轴上选择并删除操作记录。</p>}
            </div>
          )}
        </div>
        <PrivacyTimeline
          events={events}
          duration={playback.duration}
          time={playback.time}
          playing={playback.playing}
          disabled={saving}
          ranges={ranges}
          onSeek={(time) => { setEditing(false); playback.seek(time); }}
          onToggle={() => { setEditing(false); playback.toggle(); }}
          onRanges={(next) => { playback.pause(); setRanges(next); }}
          actions={ranges.length > 0 ? (
          <div className="shrink-0">
            <div className="flex flex-wrap items-center gap-3">
              <Select<PrivacyTreatment>
                aria-label="选择要处理的内容"
                containerClassName="!w-28 max-w-full"
                className="!h-7 !rounded-md"
                value={treatment}
                options={[
                  { value: 'everything', label: '删除操作', description: '删除片段中的全部操作记录及其截图' },
                  { value: 'inputs', label: '删除输入', description: '删除片段中的按键和剪贴板记录' },
                  { value: 'screenshots', label: '遮盖截图', description: '遮盖片段中的全部截图' },
                ]}
                onValueChange={setTreatment}
                disabled={saving}
              />
              <Button
                variant="primary"
                className="!h-7 !rounded-md !border-[#3563ff] !bg-[#3563ff] shadow-none"
                title={`处理所选内容（${edits.length}）`}
                disabled={saving || !edits.length}
                onClick={() => { playback.pause(); setConfirmation(edits); }}
              >处理所选内容</Button>
            </div>
          </div>
        ) : null}
        />
      </div>
      <ConfirmDialog
        open={confirmation.length > 0}
        onOpenChange={(open) => { if (!open && !locked.current) setConfirmation([]); }}
        title={removing ? `删除 ${confirmation.length} 条操作记录？` : `遮盖 ${confirmation.length} 张截图？`}
        description={removing ? '这些操作记录及其截图将从本地录制中永久删除，无法恢复。已经分享的副本不会改变。' : '选中的图像区域及对应局部放大图将永久遮盖，无法恢复。文字记录和已经分享的副本不会改变。'}
        confirmText={saving ? '正在保存…' : removing ? '删除记录' : '保存遮盖'}
        loading={saving}
        onConfirm={() => { void save(); }}
      />
    </section>
  );
}
