import { useEffect, useRef, useState } from "react";
import { aiApi } from "./api";
import type { AiConfig, ConfigView } from "./model";
/** 配置读取和保存串行；保存后立即清除前端密钥草稿。 */
export function useAiSettings() {
  const generation = useRef(0);
  const [view, setView] = useState<ConfigView | null>(null);
  const [config, setConfig] = useState<AiConfig | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  async function load() {
    const current = ++generation.current;
    setBusy(true);
    setError(null);
    try {
      const value = await aiApi.config();
      if (generation.current === current) {
        setView(value);
        setConfig(value.config);
      }
    } catch (cause) {
      if (generation.current === current) setError(String(cause));
    } finally {
      if (generation.current === current) setBusy(false);
    }
  }
  useEffect(() => {
    void load();
    return () => {
      generation.current++;
    };
  }, []);
  function change(value: AiConfig) {
    setConfig(value);
    setSaved(false);
  }
  async function save(clearKey = false) {
    if (!config || busy) return;
    const current = ++generation.current;
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      const value = await aiApi.save(config, clearKey ? "" : key || null);
      if (generation.current === current) {
        setView(value);
        setConfig(value.config);
        setKey("");
        setSaved(true);
      }
    } catch (cause) {
      if (generation.current === current) setError(String(cause));
    } finally {
      if (generation.current === current) setBusy(false);
    }
  }
  const dirty =
    !!key || JSON.stringify(config) !== JSON.stringify(view?.config ?? null);
  return {
    view,
    config,
    key,
    setKey,
    busy,
    error,
    saved,
    dirty,
    change,
    save,
    load,
  };
}
