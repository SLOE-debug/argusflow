import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

/** 本机开发服务器固定使用 IPv4，避免 WebView2 优先连接未监听的 IPv6 localhost。 */
const DEV_SERVER_HOST = '127.0.0.1' as const;

/**
 * 不属于前端模块图的工作区目录。
 *
 * Vite 的 Chokidar 默认不会读取 `.gitignore`。若不显式排除，启动后会递归遍历 Rust
 * `target`、Python Conda 环境和运行诊断目录，仓库增长后会持续抢占首个页面请求的磁盘 I/O。
 */
const DEV_SERVER_IGNORED_PATHS = [
  '**/.argusflow/**',
  '**/crates/**',
  '**/src-tauri/**',
  '**/target/**',
  '**/workers/**',
] as const;

/**
 * 首屏中必须由 Vite 转换为浏览器 ESM 的 React CommonJS 入口。
 *
 * Tauri API 与逐图标 Lucide 模块均可直接作为 ESM 加载；Monaco、Shiki 与 Zustand 仅在
 * 工作台按需加载。把它们放入这份清单会重新扩大冷启动预构建范围。
 */
const STARTUP_DEPENDENCIES = [
  'react',
  'react-dom/client',
  'react/jsx-dev-runtime',
  'react/jsx-runtime',
] as const;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  optimizeDeps: {
    /** 仅预构建首屏已知依赖，避免扫描工作台里的 Monaco 与 Shiki 模块图。 */
    include: [...STARTUP_DEPENDENCIES],
    noDiscovery: true,
  },
  server: {
    port: 5173,
    strictPort: true,
    host: DEV_SERVER_HOST,
    /** 原始 HTML 已内联启动页，不应等待 React 模块图的后台预转换。 */
    preTransformRequests: false,
    watch: {
      ignored: [...DEV_SERVER_IGNORED_PATHS],
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    css: true,
  },
});
