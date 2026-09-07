import { useCallback, useEffect, useRef, useState } from 'react';
import * as api from './api';
import { IDLE_RECORDER_STATUS, type CompletedRecording, type RecorderStatus,
  type RecordingSummary } from './model';

/** IPC 操作互斥，状态恢复和保存重试独立于面板是否可见。 */
export function useRecorder() {
  const available = api.recorderAvailable();
  const [status, setStatus] = useState<RecorderStatus>(IDLE_RECORDER_STATUS);
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState<'start' | 'stop' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [completed, setCompleted] = useState<CompletedRecording | null>(null);
  const [history, setHistory] = useState<readonly RecordingSummary[]>([]);
  const [loadingHistory, setLoadingHistory] = useState(false);
  /** 防止同一帧双击、过期轮询与卸载后的异步结果覆盖。 */
  const busy = useRef(false);
  const mounted = useRef(false);
  const revision = useRef(0);
  const historyRequest = useRef(0);

  useEffect(() => {
    mounted.current = true;
    if (!available) return () => { mounted.current = false; };
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      const epoch = revision.current;
      try {
        if (!busy.current) {
          const snapshot = await api.getRecordingStatus();
          if (!disposed && epoch === revision.current) {
            setStatus(snapshot);
            setReady(true);
            setConnectionError(null);
          }
        }
      } catch {
        if (!disposed && epoch === revision.current) {
          setReady(false);
          setConnectionError('无法读取录制状态，正在重连。');
        }
      } finally {
        if (!disposed) timer = setTimeout(() => void poll(), 1000);
      }
    };
    void poll();
    return () => { disposed = true; mounted.current = false; clearTimeout(timer); };
  }, [available]);

  const refreshHistory = useCallback(async () => {
    if (!available) return;
    try {
      const items = await api.listRecordings();
      if (mounted.current) setHistory(items);
    } catch {
      if (mounted.current) setError('无法读取录制历史，请刷新重试。');
    }
  }, [available]);

  const operate = useCallback(async (operation: 'start' | 'stop') => {
    if (!available || busy.current) return;
    busy.current = true;
    revision.current += 1;
    historyRequest.current += 1;
    setLoadingHistory(false);
    setPending(operation);
    setError(null);
    try {
      if (operation === 'start') {
        const snapshot = await api.startRecording();
        if (mounted.current) { setStatus(snapshot); setCompleted(null); setReady(true); }
      } else {
        const recording = await api.stopRecording();
        if (mounted.current) { setStatus(IDLE_RECORDER_STATUS); setCompleted(recording); setReady(true); }
        await refreshHistory();
      }
    } catch (cause) {
      if (mounted.current) {
        const reason = cause instanceof Error ? cause.message : typeof cause === 'string' ? cause : '桌面服务未响应';
        setError(`${operation === 'start' ? '开始录制失败' : '停止或保存失败'}：${reason}`);
        // 停止错误后必须重读状态，区分 Hook 未停止和文件保存失败。
        try {
          const snapshot = await api.getRecordingStatus();
          if (mounted.current) { setStatus(snapshot); setReady(true); }
        } catch { if (mounted.current) setReady(false); }
      }
    } finally {
      busy.current = false;
      revision.current += 1;
      if (mounted.current) setPending(null);
    }
  }, [available, refreshHistory]);

  const selectHistory = useCallback(async (id: string) => {
    if (busy.current) return;
    const request = ++historyRequest.current;
    setLoadingHistory(true);
    setError(null);
    try {
      const recording = await api.getRecording(id);
      if (mounted.current && request === historyRequest.current) setCompleted(recording);
    } catch {
      if (mounted.current && request === historyRequest.current) setError('无法打开这次录制，请检查本地文件后重试。');
    } finally {
      if (mounted.current && request === historyRequest.current) setLoadingHistory(false);
    }
  }, []);

  return { available, ready, status, pending, error: connectionError ?? error, completed, history, loadingHistory,
    start: () => operate('start'), stop: () => operate('stop'), refreshHistory, selectHistory };
}

/** 界面只依赖此控制器公开契约。 */
export type RecorderController = ReturnType<typeof useRecorder>;
