import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

type DesktopWindowConfiguration = Readonly<{
  /** Tauri IPC 与前端共同使用的稳定窗口标签。 */
  label: string;
  /** 主窗口先隐藏，避免 WebView2 的 `about:blank` 占位文档暴露给用户。 */
  visible?: boolean;
}>;

type DesktopConfiguration = Readonly<{
  /** 桌面应用窗口清单。 */
  app: Readonly<{
    windows: ReadonlyArray<DesktopWindowConfiguration>;
  }>;
}>;

type DesktopCapability = Readonly<{
  /** 当前窗口允许调用的 Tauri 命令权限。 */
  permissions: ReadonlyArray<string>;
}>;

describe('desktop startup window configuration', () => {
  it('defines one initially hidden main window and no second window', () => {
    /** 直接检查 Tauri 实际消费的配置，防止以后重新引入第二窗口。 */
    const configurationPath = resolve(process.cwd(), 'src-tauri', 'tauri.conf.json');
    const configuration = JSON.parse(
      readFileSync(configurationPath, 'utf8'),
    ) as DesktopConfiguration;

    expect(configuration.app.windows).toHaveLength(1);
    expect(configuration.app.windows[0]).toMatchObject({
      label: 'main',
      visible: false,
    });
  });

  it('renders the static loading page before the React module executes', () => {
    /** 原始 HTML 必须先包含 Loading，不能等待 React 模块图下载和执行。 */
    const documentPath = resolve(process.cwd(), 'index.html');
    const document = readFileSync(documentPath, 'utf8');
    const loadingPosition = document.indexOf('id="argusflow-boot-loading"');
    const reactModulePosition = document.indexOf('/src/main.tsx');

    expect(loadingPosition).toBeGreaterThan(-1);
    expect(reactModulePosition).toBeGreaterThan(loadingPosition);
    expect(document).toContain('@keyframes argusflow-boot-spin');
  });

  it('allows the bootstrap module to show the main window', () => {
    /** 显示权限必须随隐藏启动策略一起保留，否则窗口会永久不可见。 */
    const capabilityPath = resolve(
      process.cwd(),
      'src-tauri',
      'capabilities',
      'default.json',
    );
    const capability = JSON.parse(
      readFileSync(capabilityPath, 'utf8'),
    ) as DesktopCapability;

    expect(capability.permissions).toContain('core:window:allow-show');
  });

  it('reveals the window from a native document initialization script', () => {
    /** 首屏显示不得重新引入会阻塞 Vite 首请求的前端 Tauri Window 模块。 */
    const bootstrapPath = resolve(
      process.cwd(),
      'src-tauri',
      'src',
      'window_bootstrap.rs',
    );
    const bootstrap = readFileSync(bootstrapPath, 'utf8');

    expect(bootstrap).toContain("getElementById('argusflow-boot-loading')");
    expect(bootstrap).toContain("invoke('plugin:window|show'");
  });
});
