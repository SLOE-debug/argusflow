import { useEffect, useState } from 'react';

import type { AqlInspection, AqlQuery } from './contracts';
import { inspectAql, isDesktopRuntime } from './workflowApi';

/** AQL 实时检查在编辑器中的请求状态。 */
export type AqlInspectionState =
  | { phase: 'loading'; inspection: null; message: null }
  | { phase: 'ready'; inspection: AqlInspection; message: null }
  | { phase: 'unavailable'; inspection: null; message: string };

/** 输入停止短暂间隔后调用 Rust parser，且丢弃过期响应。 */
export function useAqlInspection(query: AqlQuery): AqlInspectionState {
  const [state, setState] = useState<AqlInspectionState>(() => (
    isDesktopRuntime()
      ? { phase: 'loading', inspection: null, message: null }
      : {
          phase: 'unavailable',
          inspection: null,
          message: '实时 AQL 分析仅在 ArgusFlow 桌面应用中可用。',
        }
  ));

  useEffect(() => {
    if (!isDesktopRuntime()) {
      setState({
        phase: 'unavailable',
        inspection: null,
        message: '实时 AQL 分析仅在 ArgusFlow 桌面应用中可用。',
      });
      return;
    }

    let active = true;
    setState({ phase: 'loading', inspection: null, message: null });
    /** 避免每次键入都跨越 IPC，同时保持诊断足够及时。 */
    const debounceTimer = window.setTimeout(() => {
      void inspectAql(query)
        .then((inspection) => {
          if (active) {
            setState({ phase: 'ready', inspection, message: null });
          }
        })
        .catch((error: unknown) => {
          if (active) {
            setState({
              phase: 'unavailable',
              inspection: null,
              message: error instanceof Error ? error.message : String(error),
            });
          }
        });
    }, 220);

    return () => {
      active = false;
      window.clearTimeout(debounceTimer);
    };
  }, [query]);

  return state;
}
