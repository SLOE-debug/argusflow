import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // 默认 HTML 搜索会遍历 target 中的 Tauri 打包产物。
  optimizeDeps: { entries: ['index.html'] },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    watch: {
      // Rust 由 Tauri/Cargo 监听，避免 Vite 重复遍历源码和构建目录。
      ignored: [
        '**/target/**',
        '**/src-tauri/**',
        '**/crates/**',
        '**/.deps/**',
        '**/.cache/**',
        '**/.pnpm-store/**',
      ],
    },
  },
});
