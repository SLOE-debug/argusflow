/** 正常可见的 Chrome 窗口；启用沙箱，避免不受支持参数与自动化提示条。 */
export function launchOptions(executablePath) {
  return {
    executablePath, channel: 'chrome', headless: false, viewport: null,
    chromiumSandbox: true,
    ignoreDefaultArgs: ['--enable-automation', '--disable-blink-features=AutomationControlled'],
    args: ['--start-maximized', '--no-first-run', '--no-default-browser-check'],
    timeout: 30_000,
  };
}
