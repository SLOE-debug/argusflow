import { Channel, invoke } from "@tauri-apps/api/core";
import type { AiConfig, AiProgress, AiResult, ConfigView } from "./model";
/** 唯一 IPC 边界；不把 API Key 写入 localStorage 或前端日志。 */
export const aiApi = {
  config: () => invoke<ConfigView>("ai_config"),
  save: (config: AiConfig, apiKey: string | null) =>
    invoke<ConfigView>("ai_save_config", {
      update: { config, api_key: apiKey },
    }),
  analyze: (
    id: string,
    directory: string,
    onProgress: (event: AiProgress) => void,
  ) => {
    const channel = new Channel<AiProgress>();
    channel.onmessage = onProgress;
    return invoke<AiResult>("ai_analyze", { id, directory, channel });
  },
  cancel: (id: string) => invoke<void>("ai_cancel", { id }),
};
