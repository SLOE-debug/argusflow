import { useEffect, useRef, useState } from 'react';
import type { CompletedRecording } from './model';
import { editRecordingPrivacy, ocrPrivacyRect, privacyKeywords, recognizePrivacyImage, recordingSearchText, type PrivacyEdit, type PrivacyResult } from './privacy';

/** 隐私搜索、选择与持久化；组件只编排展示，不保存派生的录制副本。 */
export function useRecordingPrivacy(recording: CompletedRecording, onSaved: (recording: CompletedRecording) => void) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<readonly PrivacyResult[]>([]);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState('输入姓名、电话等关键词，或直接查找全部内容。多个关键词请用逗号分隔。');
  /** 组件卸载或录制切换后忽略尚未完成的 OCR。 */
  const generation = useRef(0);
  const locked = useRef(false);
  /** 取消在当前 OCR 返回后生效，保留已找到的内容。 */
  const cancelled = useRef(false);
  useEffect(() => () => { generation.current += 1; }, []);
  const search = async () => {
    if (locked.current) return;
    locked.current = true;
    cancelled.current = false;
    const request = ++generation.current;
    setBusy(true);
    setSelected(new Set());
    setResults([]);
    const words = privacyKeywords(query);
    const matches = (text: string) => words.length === 0 || words.some((word) => text.toLocaleLowerCase().includes(word));
    const found: PrivacyResult[] = [];
    const content = recordingSearchText(recording.trace.timeline.events);
    let failed = 0;
    try {
      for (const [index, event] of recording.trace.timeline.events.entries()) {
        if (request !== generation.current || cancelled.current) return;
        const text = content.get(event.sequence) ?? '';
        // 留空时允许删除没有文字的鼠标等记录。
        if (words.length === 0 || (text && matches(text))) found.push({ id: `text:${event.sequence}`, sequence: event.sequence, source: 'text', text: text || '这条操作记录没有文字内容' });
        setNotice(`正在检查 ${index + 1}/${recording.trace.timeline.events.length} 条操作记录…`);
        const images = [ ['window', event.evidence?.screenshot], ['target', event.evidence?.click_target] ] as const;
        for (const [kind, shot] of images) {
          if (cancelled.current) return;
          if (!shot) continue;
          try {
            const items = await recognizePrivacyImage(recording.trace.recording_id, event.sequence, kind);
            if (request !== generation.current) return;
            items.forEach((item, itemIndex) => {
              const rect = ocrPrivacyRect(item, shot.width, shot.height);
              if (rect && matches(item.raw_text)) found.push({ id: `${kind}:${event.sequence}:${itemIndex}`, sequence: event.sequence, source: 'image', kind, rect, text: item.raw_text });
            });
          } catch { failed += 1; }
        }
        setResults([...found]);
      }
      setNotice(`找到 ${found.length} 项内容。${failed ? `${failed} 张截图未能识别，可手动框选。` : '文字识别可能有遗漏，请再查看截图。'}`);
    } finally {
      locked.current = false;
      if (request === generation.current) setBusy(false);
    }
  };
  const save = async (edits: readonly PrivacyEdit[]) => {
    if (locked.current || !edits.length) return;
    locked.current = true;
    setBusy(true);
    setSaving(true);
    try {
      const updated = await editRecordingPrivacy(recording.trace.recording_id, edits);
      onSaved(updated);
      setResults([]);
      setSelected(new Set());
      setNotice('已保存隐私处理。可以继续检查其他内容。');
    } catch { setNotice('隐私处理未完成，部分内容可能已处理。请重新打开录制检查后重试。'); }
    finally { locked.current = false; setBusy(false); setSaving(false); }
  };
  const toggle = (id: string) => setSelected((previous) => {
    const next = new Set(previous);
    if (next.has(id)) next.delete(id); else next.add(id);
    return next;
  });
  const cancel = () => { cancelled.current = true; setNotice('正在停止查找，当前截图处理完成后即可继续操作。'); };
  return { query, setQuery, results, selected, setSelected, toggle, busy, saving, notice, search, save, cancel };
}
