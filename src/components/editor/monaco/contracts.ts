import type * as Monaco from "monaco-editor/esm/vs/editor/editor.api.js";
/** 语言只负责语法注册；模型与编辑器生命周期由共享组件管理。 */
export interface EditorLanguage {
  readonly id: string;
  readonly register: (
    api: typeof Monaco,
    model: Monaco.editor.ITextModel,
    composing: () => boolean,
  ) => Monaco.IDisposable;
}
