import type { EditorPosition, EditorRange } from '../../workflow/contracts';
import type {
  AqlLanguageService,
  CompletionItem,
  Hover,
  LanguageDocument,
  TextEdit,
} from './types';

/** `wasm-bindgen --target web` 生成模块的最小强类型边界。 */
type WasmLanguageModule = Readonly<{
  default: () => Promise<unknown>;
  inspect: (source: string) => LanguageDocument;
  completions: (source: string, line: number, column: number) => readonly CompletionItem[];
  hover: (source: string, line: number, column: number) => Hover | null;
  bracket_pair: (source: string, line: number, column: number) => readonly EditorRange[] | null;
  code_actions: (source: string) => readonly TextEdit[];
}>;

/** 生成后的 WASM/JS 位于 Vite public 目录，由显式构建脚本产生。 */
const WASM_MODULE_URL = '/aql-wasm/argusflow_query_wasm.js';

/** 复用唯一初始化 Promise，避免多个编辑器重复实例化 WASM。 */
let languageServicePromise: Promise<AqlLanguageService> | null = null;

/** 加载由 `argusflow-query-wasm` 导出的同源 Rust 语言服务。 */
export function loadAqlLanguageService(): Promise<AqlLanguageService> {
  languageServicePromise ??= loadWasmModule();
  return languageServicePromise;
}

/** 测试或热更新失败后允许重新发起初始化。 */
export function resetAqlLanguageService(): void {
  languageServicePromise = null;
}

async function loadWasmModule(): Promise<AqlLanguageService> {
  // Vite 不应在前端构建阶段解析由 Rust 构建脚本生成的模块。
  const loadedModule = await import(/* @vite-ignore */ WASM_MODULE_URL) as WasmLanguageModule;
  await loadedModule.default();

  return {
    inspect: (source) => loadedModule.inspect(source),
    completions: (source, position: EditorPosition) => loadedModule.completions(
      source,
      position.line,
      position.utf16_column,
    ),
    hover: (source, position: EditorPosition) => loadedModule.hover(
      source,
      position.line,
      position.utf16_column,
    ),
    bracketPair: (source, position: EditorPosition) => loadedModule.bracket_pair(
      source,
      position.line,
      position.utf16_column,
    ),
    codeActions: (source) => loadedModule.code_actions(source),
  };
}
