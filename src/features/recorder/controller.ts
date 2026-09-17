import { useCallback, useEffect, useRef, useState } from "react";
import { recorderApi } from "./api";
import { appendWindow } from "./buffer";
import type {
  Cursor,
  Phase,
  RecorderStatus,
  RecordingEntry,
  RecordingRecord,
} from "./model";
/** 全局录制入口的领域状态；异步目录切换用代际拒绝迟到结果。 */
export function useRecorder() {
  const [status, setStatus] = useState<RecorderStatus | null>(null);
  const [entries, setEntries] = useState<readonly RecordingEntry[]>([]);
  const [opened, setOpened] = useState<RecordingEntry | null>(null);
  const [records, setRecords] = useState<readonly RecordingRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [end, setEnd] = useState(false);
  const [following, setFollowing] = useState(true);
  const cursor = useRef<Cursor>({ offset: 0, sequence: 0 });
  const generation = useRef(0);
  const loading = useRef<number | null>(null);
  const mounted = useRef(true);
  const refresh = useCallback(async () => {
    const values = await recorderApi.list();
    if (mounted.current) setEntries(values);
  }, []);
  const safely = useCallback(async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (failure) {
      if (mounted.current) setError(String(failure));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const value = await recorderApi.status();
        if (!cancelled) setStatus(value);
      } catch (failure) {
        if (!cancelled) setError(String(failure));
      }
      if (!cancelled) timer = setTimeout(() => void poll(), 500);
    };
    void poll();
    void refresh().catch((failure) => {
      if (!cancelled) setError(String(failure));
    });
    return () => {
      cancelled = true;
      mounted.current = false;
      generation.current++;
      clearTimeout(timer);
    };
  }, [refresh]);
  const open = useCallback(async (directory: string) => {
    const current = ++generation.current;
    const session = await recorderApi.open(directory);
    if (current !== generation.current || !mounted.current) return;
    cursor.current = { offset: 0, sequence: 0 };
    setRecords([]);
    setEnd(false);
    setOpened({ session, directory });
  }, []);
  const loadMore = useCallback(async () => {
    const current = generation.current;
    if (!opened || loading.current === current) return;
    loading.current = current;
    try {
      // 回看一次读取最多8页，避免原始鼠标事件占满首页却看不到关联后图。
      for (let batch = 0; batch < 8; batch++) {
        const page = await recorderApi.read(opened.directory, cursor.current);
        if (current !== generation.current || !mounted.current) return;
        cursor.current = page.cursor;
        setEnd(page.end);
        setRecords((previous) => appendWindow(previous, page.records));
        if (page.tail) setError(`已读取完整记录，尾部尚未完整：${page.tail}`);
        if (page.end || page.tail || !page.records.length) break;
      }
    } finally {
      if (loading.current === current) loading.current = null;
    }
  }, [opened]);
  useEffect(() => {
    if (opened) void loadMore().catch((failure) => setError(String(failure)));
  }, [opened, loadMore]);
  useEffect(() => {
    if (!following || !opened || opened.session.id !== status?.session) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        await loadMore();
      } catch (failure) {
        if (!cancelled) setError(String(failure));
      }
      if (!cancelled) timer = setTimeout(() => void poll(), 500);
    };
    timer = setTimeout(() => void poll(), 500);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [following, opened, status?.session, loadMore]);
  useEffect(() => {
    if (!opened || end || records.some(({ data }) => "Interaction" in data))
      return;
    const timer = setTimeout(
      () => void loadMore().catch((failure) => setError(String(failure))),
      100,
    );
    return () => clearTimeout(timer);
  }, [opened, end, records, loadMore]);
  const start = () =>
    safely(async () => {
      await recorderApi.start();
      await refresh();
      const current = await recorderApi.status();
      const sessions = await recorderApi.list();
      const entry = sessions.find(
        (item) => item.session.id === current.session,
      );
      if (entry) await open(entry.directory);
    });
  const transition = (phase: Phase) =>
    safely(async () => {
      await recorderApi.transition(phase);
      await refresh();
    });
  return {
    status,
    entries,
    opened,
    records,
    error,
    busy,
    end,
    following,
    setFollowing,
    start,
    transition,
    safely,
    refresh,
    open,
    loadMore,
  };
}
