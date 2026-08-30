/**
 * Lucide 的逐图标 ESM 文件没有随包发布独立声明；每个默认导出都遵循统一的
 * `LucideIcon` 组件契约。逐图标导入可避免开发服务器解析包含数千模块的总入口。
 */
declare module 'lucide-react/dist/esm/icons/*.mjs' {
  import type { LucideIcon } from 'lucide-react';

  const Icon: LucideIcon;
  export default Icon;
}
