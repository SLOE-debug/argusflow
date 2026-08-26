import type { BrowserSpec } from './contracts';

/** 新建 Browser 节点使用的隔离 Chromium 默认契约。 */
export function createDefaultBrowserSpec(): BrowserSpec {
  return {
    executable_path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    initial_url: 'https://www.baidu.com/',
    launch_timeout_ms: 15_000,
  };
}
