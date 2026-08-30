/** 启动门控结论；三项本地能力全部就绪后才进入 Home。 */
export type StartupReadiness = 'loading' | 'ready' | 'blocked';

/** 后端发布的当前主要启动阶段。 */
export type StartupPhase =
  | 'starting_runtime'
  | 'initializing_capture'
  | 'selecting_ocr_device'
  | 'loading_small_model'
  | 'warming_small_model'
  | 'loading_medium_model'
  | 'warming_medium_model'
  | 'ready'
  | 'failed';

/** 单项能力的初始化生命周期。 */
export type StartupComponentLifecycle =
  | 'pending'
  | 'initializing'
  | 'warming'
  | 'ready'
  | 'failed';

/** Paddle 最终选择的强类型推理设备。 */
export type WorkerDevice =
  | Readonly<{ kind: 'cpu' }>
  | Readonly<{ kind: 'cuda'; index: number }>;

/** WGC 或 OCR 档位的安全状态摘要。 */
export type StartupComponentStatus = Readonly<{
  lifecycle: StartupComponentLifecycle;
  message: string | null;
}>;

/** 启动页、工作台门控和状态栏共享的运行时快照。 */
export type StartupSnapshot = Readonly<{
  readiness: StartupReadiness;
  phase: StartupPhase;
  completedSteps: number;
  totalSteps: number;
  capture: StartupComponentStatus;
  smallOcr: StartupComponentStatus;
  mediumOcr: StartupComponentStatus;
  device: WorkerDevice | null;
  degradationReason: string | null;
}>;

/** Web 预览和 Tauri 首次 IPC 返回前使用的最小初始快照。 */
export const INITIAL_STARTUP_SNAPSHOT: StartupSnapshot = {
  readiness: 'loading',
  phase: 'starting_runtime',
  completedSteps: 0,
  totalSteps: 3,
  capture: { lifecycle: 'pending', message: null },
  smallOcr: { lifecycle: 'pending', message: null },
  mediumOcr: { lifecycle: 'pending', message: null },
  device: null,
  degradationReason: null,
};

/** Vite 浏览器预览不具备桌面能力，因此提供稳定的可进入工作台快照。 */
export const BROWSER_STARTUP_SNAPSHOT: StartupSnapshot = {
  ...INITIAL_STARTUP_SNAPSHOT,
  readiness: 'ready',
  phase: 'ready',
  completedSteps: 3,
  capture: { lifecycle: 'ready', message: null },
  smallOcr: { lifecycle: 'ready', message: null },
  mediumOcr: { lifecycle: 'ready', message: null },
  device: { kind: 'cpu' },
};
