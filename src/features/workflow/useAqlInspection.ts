import { useEffect, useState } from 'react';

import type { AqlInspection, AutomationTarget } from './contracts';
import { inspectAql, isDesktopRuntime } from './workflowApi';

/** 非桌面预览无法读取真实运行环境时显示的说明。 */
const DESKTOP_RUNTIME_MESSAGE = '请在 ArgusFlow 桌面应用中检查运行环境。';
/** Runtime 检查失败时给用户的下一步提示。 */
const INSPECTION_ERROR_MESSAGE = '运行环境检查暂不可用，请稍后重试。';

/** Runtime Planner Explain 在编辑器中的请求状态。 */
export type AqlInspectionState =
  | { phase: 'loading'; inspection: null; message: null }
  | { phase: 'ready'; inspection: AqlInspection; message: null }
  | { phase: 'unavailable'; inspection: null; message: string };

/** 输入稳定后调用桌面 Runtime Planner，且丢弃过期响应。 */
export function useAqlInspection(
  target: AutomationTarget,
): AqlInspectionState {
  const [state, setState] = useState<AqlInspectionState>(() => (
    isDesktopRuntime()
      ? { phase: 'loading', inspection: null, message: null }
      : {
          phase: 'unavailable',
          inspection: null,
          message: DESKTOP_RUNTIME_MESSAGE,
        }
  ));

  useEffect(() => {
    if (!isDesktopRuntime()) {
      setState({
        phase: 'unavailable',
        inspection: null,
        message: DESKTOP_RUNTIME_MESSAGE,
      });
      return;
    }

    let active = true;
    setState({ phase: 'loading', inspection: null, message: null });
    /** Runtime planning 依赖系统上下文，稳定输入后再跨越 IPC。 */
    const debounceTimer = window.setTimeout(() => {
      void inspectAql(target)
        .then((inspection) => {
          if (active) {
            setState({ phase: 'ready', inspection, message: null });
          }
        })
        .catch(() => {
          if (active) {
            setState({
              phase: 'unavailable',
              inspection: null,
              message: INSPECTION_ERROR_MESSAGE,
            });
          }
        });
    }, 220);

    return () => {
      active = false;
      window.clearTimeout(debounceTimer);
    };
  }, [target]);

  return state;
}
