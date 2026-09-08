import { useEffect, useState } from 'react';
import { readRecordingScreenshot } from './api';

/** URL 只在当前事件预览生命周期内有效。 */
type ImageState = Readonly<{ type: 'loading' }> | Readonly<{ type: 'failed' }> | Readonly<{ type: 'ready'; url: string }>;

/** 按事件身份读取 PNG，并回收过期 URL。 */
export function useEvidenceImage(recordingId: string, sequence: number, kind: 'window' | 'crop' | 'target' | 'target_crop') {
  const [state, setState] = useState<ImageState>({ type: 'loading' });
  useEffect(() => {
    let cancelled = false;
    let imageUrl: string | undefined;
    setState({ type: 'loading' });
    void readRecordingScreenshot(recordingId, sequence, kind).then((bytes) => {
      if (cancelled) return;
      imageUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
      setState({ type: 'ready', url: imageUrl });
    }).catch(() => { if (!cancelled) setState({ type: 'failed' }); });
    return () => { cancelled = true; if (imageUrl) URL.revokeObjectURL(imageUrl); };
  }, [recordingId, sequence, kind]);
  return state;
}
