import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';

import { scheduleAfterReactCommit } from './scheduleAfterReactCommit';
import {
  beginRuntimeInitialization,
  getStartupStatus,
  hasDesktopRuntime,
  retryStartup,
} from './startupApi';
import { BROWSER_STARTUP_SNAPSHOT, INITIAL_STARTUP_SNAPSHOT } from './model';
import type { StartupSnapshot } from './model';

const STARTUP_STATUS_EVENT = 'argusflow://startup-status';

/** 订阅后端能力初始化，并提供无页面刷新的重试操作。 */
export function useStartupStatus() {
  const [status, setStatus] = useState<StartupSnapshot>(
    hasDesktopRuntime() ? INITIAL_STARTUP_SNAPSHOT : BROWSER_STARTUP_SNAPSHOT,
  );
  const [retrying, setRetrying] = useState(false);
  const [initializationError, setInitializationError] = useState<string | null>(null);
  /** 后端会持续发布健康快照；内容没有变化时保留旧引用，避免整棵工作台重复渲染。 */
  const updateStatus = useCallback((nextStatus: StartupSnapshot) => {
    setStatus((currentStatus) => (
      startupSnapshotsEqual(currentStatus, nextStatus) ? currentStatus : nextStatus
    ));
  }, []);

  useEffect(() => {
    if (!hasDesktopRuntime()) return undefined;
    let active = true;
    /** 先订阅事件再读取快照，避免漏掉很快完成的 WGC 初始化。 */
    const unlistenPromise = listen<StartupSnapshot>(STARTUP_STATUS_EVENT, (event) => {
      if (active) updateStatus(event.payload);
    });
    void getStartupStatus().then((snapshot) => {
      if (active) updateStatus(snapshot);
    });
    /** effect 返回后立即启动后台能力；宏任务可被 StrictMode 探测阶段安全取消。 */
    const cancelInitialization = scheduleAfterReactCommit(() => {
      void beginRuntimeInitialization()
        .then((snapshot) => {
          if (active) updateStatus(snapshot);
        })
        .catch((error: unknown) => {
          if (active) {
            setInitializationError(error instanceof Error ? error.message : String(error));
          }
        });
    });
    return () => {
      active = false;
      cancelInitialization();
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [updateStatus]);

  const retry = useCallback(async () => {
    setInitializationError(null);
    setRetrying(true);
    try {
      updateStatus(await retryStartup());
    } finally {
      setRetrying(false);
    }
  }, [updateStatus]);

  return { status, retrying, retry, initializationError } as const;
}

/** 比较两次健康轮询的能力状态，忽略后端反序列化产生的新对象引用。 */
function startupSnapshotsEqual(
  current: StartupSnapshot,
  next: StartupSnapshot,
): boolean {
  return current.readiness === next.readiness
    && current.phase === next.phase
    && current.completedSteps === next.completedSteps
    && current.totalSteps === next.totalSteps
    && startupComponentsEqual(current.capture, next.capture)
    && startupComponentsEqual(current.smallOcr, next.smallOcr)
    && startupComponentsEqual(current.mediumOcr, next.mediumOcr)
    && workerDevicesEqual(current.device, next.device)
    && current.degradationReason === next.degradationReason;
}

/** 比较单项捕获或 OCR 生命周期及其可显示说明。 */
function startupComponentsEqual(
  current: StartupSnapshot['capture'],
  next: StartupSnapshot['capture'],
): boolean {
  return current.lifecycle === next.lifecycle && current.message === next.message;
}

/** 比较可选推理设备，并在 CUDA 模式下包含设备序号。 */
function workerDevicesEqual(
  current: StartupSnapshot['device'],
  next: StartupSnapshot['device'],
): boolean {
  if (current === null || next === null) return current === next;
  if (current.kind !== next.kind) return false;
  if (current.kind === 'cpu') return true;
  return next.kind === 'cuda' && current.index === next.index;
}
