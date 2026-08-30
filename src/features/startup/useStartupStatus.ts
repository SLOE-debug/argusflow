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

  useEffect(() => {
    if (!hasDesktopRuntime()) return undefined;
    let active = true;
    /** 先订阅事件再读取快照，避免漏掉很快完成的 WGC 初始化。 */
    const unlistenPromise = listen<StartupSnapshot>(STARTUP_STATUS_EVENT, (event) => {
      if (active) setStatus(event.payload);
    });
    void getStartupStatus().then((snapshot) => {
      if (active) setStatus(snapshot);
    });
    /** effect 返回后立即启动后台能力；宏任务可被 StrictMode 探测阶段安全取消。 */
    const cancelInitialization = scheduleAfterReactCommit(() => {
      void beginRuntimeInitialization()
        .then((snapshot) => {
          if (active) setStatus(snapshot);
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
  }, []);

  const retry = useCallback(async () => {
    setInitializationError(null);
    setRetrying(true);
    try {
      setStatus(await retryStartup());
    } finally {
      setRetrying(false);
    }
  }, []);

  return { status, retrying, retry, initializationError } as const;
}
