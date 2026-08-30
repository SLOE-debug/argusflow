import { lazy, Suspense, useEffect, useState } from 'react';

import { StartupScreen } from './components/shell/startup';
import { useStartupStatus } from './features/startup';

/** 工作台大包只在全部能力就绪或用户明确进入降级模式后加载。 */
const App = lazy(() => import('./App'));

/** 负责首帧启动体验、能力门控和工作台按需加载。 */
export default function AppBootstrap() {
  const {
    status,
    retrying,
    retry,
    initializationError,
  } = useStartupStatus();
  const [enteredWorkbench, setEnteredWorkbench] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);

  useEffect(() => {
    if (status.readiness === 'ready') setEnteredWorkbench(true);
  }, [status.readiness]);

  /** 重试失败留在当前卡片展示，不制造未处理的 Promise 拒绝。 */
  const handleRetry = async () => {
    setRetryError(null);
    try {
      await retry();
    } catch (error) {
      setRetryError(error instanceof Error ? error.message : String(error));
    }
  };

  const startupFallback = (
    <StartupScreen
      status={status}
      retrying={retrying}
      errorMessage={retryError ?? initializationError}
      onRetry={() => void handleRetry()}
      onContinueDegraded={() => setEnteredWorkbench(true)}
    />
  );
  if (!enteredWorkbench) return startupFallback;

  return (
    <Suspense fallback={startupFallback}>
      <App
        startupStatus={status}
        executionEnabled={status.readiness === 'ready'}
      />
    </Suspense>
  );
}
