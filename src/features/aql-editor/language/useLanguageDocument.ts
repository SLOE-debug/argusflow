import { useEffect, useState } from 'react';

import { loadAqlLanguageService } from './LanguageClient';
import type { AqlLanguageService, LanguageDocument } from './types';

/** AQL WASM 初始化和文档检查状态。 */
export type LanguageDocumentState =
  | { phase: 'loading'; document: null; service: null; message: null }
  | { phase: 'ready'; document: LanguageDocument; service: AqlLanguageService; message: null }
  | { phase: 'unavailable'; document: null; service: null; message: string };

/** 每次输入直接在 WebView 内调用 Rust WASM，不经过 debounce 或 Tauri IPC。 */
export function useLanguageDocument(source: string): LanguageDocumentState {
  const [state, setState] = useState<LanguageDocumentState>({
    phase: 'loading',
    document: null,
    service: null,
    message: null,
  });

  useEffect(() => {
    let active = true;
    void loadAqlLanguageService()
      .then((service) => {
        if (active) {
          setState({ phase: 'ready', document: service.inspect(source), service, message: null });
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setState({
            phase: 'unavailable',
            document: null,
            service: null,
            message: error instanceof Error ? error.message : String(error),
          });
        }
      });

    return () => {
      active = false;
    };
  }, [source]);

  return state;
}
