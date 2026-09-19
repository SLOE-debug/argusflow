import { useEffect, useRef, useState } from "react";
import { aiApi } from "./api";
import type { AiProgress, AiResult } from "./model";
/** 每次请求有独立身份，关闭面板或更换录制时取消并丢弃晚到结果。 */
export function useAiAnalysis(directory: string | undefined) {
  const active = useRef<string | null>(null);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<AiProgress | null>(null);
  const [result, setResult] = useState<AiResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setResult(null);
    setProgress(null);
    setError(null);
    setRunning(false);
    return () => {
      const id = active.current;
      active.current = null;
      if (id) void aiApi.cancel(id).catch(() => {});
    };
  }, [directory]);
  async function run() {
    if (!directory || active.current) return;
    const id = crypto.randomUUID();
    active.current = id;
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      const next = await aiApi.analyze(id, directory, (value) => {
        if (active.current === id) setProgress(value);
      });
      if (active.current === id) setResult(next);
    } catch (cause) {
      if (active.current === id) setError(String(cause));
    } finally {
      if (active.current === id) {
        active.current = null;
        setRunning(false);
      }
    }
  }
  async function cancel() {
    if (!active.current) return;
    try {
      await aiApi.cancel(active.current);
    } catch (cause) {
      setError(String(cause));
    }
  }
  return { running, progress, result, error, run, cancel };
}
