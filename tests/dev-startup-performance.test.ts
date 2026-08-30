import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

/** 递归收集可能包含前端运行时 import 的 TypeScript 源文件。 */
function collectTypeScriptSources(directory: string): ReadonlyArray<string> {
  /** 当前目录下需要继续遍历或检查的条目。 */
  const entries = readdirSync(directory, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) return collectTypeScriptSources(entryPath);
    return /\.(?:ts|tsx)$/.test(entry.name) ? [entryPath] : [];
  });
}

describe('development startup performance guards', () => {
  it('keeps non-frontend workspace trees out of the Vite watcher', () => {
    /** Vite 不读取 `.gitignore`，因此这些巨型目录必须留在显式 ignore 清单。 */
    const configuration = readFileSync(resolve(process.cwd(), 'vite.config.ts'), 'utf8');

    expect(configuration).toContain("'**/target/**'");
    expect(configuration).toContain("'**/workers/**'");
    expect(configuration).toContain("'**/.argusflow/**'");
    expect(configuration).toContain("'**/crates/**'");
  });

  it('limits Tailwind class detection to the frontend source tree', () => {
    /** 从仓库根自动扫描会把 Rust 与 Python 源码加入首次 CSS 转换。 */
    const stylesheet = readFileSync(resolve(process.cwd(), 'src', 'styles.css'), 'utf8');

    expect(stylesheet).toContain('@import "tailwindcss" source(".");');
  });

  it('does not load the full Lucide icon barrel at runtime', () => {
    /** 类型 import 会被编译器移除；只禁止会展开全部图标模块的运行时入口。 */
    const typeOnlyImport = /import\s+type\s+\{[^}]*\}\s+from\s+['"]lucide-react['"];?/g;
    /** 逐文件报告可以在回归时直接定位重新引入总入口的位置。 */
    const runtimeBarrelImports = collectTypeScriptSources(resolve(process.cwd(), 'src'))
      .filter((sourcePath) => {
        const source = readFileSync(sourcePath, 'utf8').replace(typeOnlyImport, '');
        return /from\s+['"]lucide-react['"]/.test(source);
      });

    expect(runtimeBarrelImports).toEqual([]);
  });
});
