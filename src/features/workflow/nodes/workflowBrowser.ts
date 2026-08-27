import type { BrowserSpec } from '../model/contracts';

/** 新建 Browser 节点使用的隔离 Chromium 默认契约。 */
export function createDefaultBrowserSpec(): BrowserSpec {
  return {
    executable_path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    acquire_mode: 'launch_isolated_cdp',
    launch_timeout_ms: 15_000,
    cleanup_policy: 'close_on_workflow_end',
  };
}
