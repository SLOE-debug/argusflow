import { invoke } from '@tauri-apps/api/core';

import { BROWSER_STARTUP_SNAPSHOT, type StartupSnapshot } from './model';

/** 判断当前页面是否运行在拥有 Tauri IPC 的桌面 WebView 中。 */
export function hasDesktopRuntime(): boolean {
  return typeof window !== 'undefined'
    && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

/** 在 React 启动页提交后立即开始 WGC 与 OCR 后台初始化。 */
export function beginRuntimeInitialization(): Promise<StartupSnapshot> {
  return hasDesktopRuntime()
    ? invoke<StartupSnapshot>('begin_runtime_initialization')
    : Promise.resolve(BROWSER_STARTUP_SNAPSHOT);
}

/** 读取当前分阶段能力启动状态。 */
export function getStartupStatus(): Promise<StartupSnapshot> {
  return hasDesktopRuntime()
    ? invoke<StartupSnapshot>('get_startup_status')
    : Promise.resolve(BROWSER_STARTUP_SNAPSHOT);
}

/** 请求后端重试失败的捕获和 OCR 初始化。 */
export function retryStartup(): Promise<StartupSnapshot> {
  return hasDesktopRuntime()
    ? invoke<StartupSnapshot>('retry_startup')
    : Promise.resolve(BROWSER_STARTUP_SNAPSHOT);
}
