import type { LanguageService } from './contracts';

/** wasm-bindgen 生成的模块负责资源初始化；Promise 只缓存加载，不缓存分析结果。 */
let loading: Promise<LanguageService> | undefined;
/** 加载原生 Rust 编译器的 WASM 版本。 */
export function loadLanguageService(): Promise<LanguageService> {
  loading ??= import('./generated/aql').then(async (module) => {
    await module.default();
    return {
      inspect: (source: string) => module.inspect(source),
      completions: (source: string, position) => module.completions(source, position.line, position.column),
      hover: (source: string, position) => module.hover(source, position.line, position.column) ?? undefined,
      importEnglish: (source: string) => module.importEnglish(source),
    } satisfies LanguageService;
  }).catch((error: unknown) => {
    loading = undefined;
    throw error;
  });
  return loading;
}
