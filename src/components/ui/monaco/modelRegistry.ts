import type * as Monaco from 'monaco-editor/editor/editor.api';

import type { MonacoApi } from './monacoLoader';

/** 带引用计数的 Monaco 文档；延迟释放允许内联与抽屉切换复用 undo 栈。 */
type ModelEntry = {
  /** 当前 URI 唯一对应的文本模型。 */
  readonly model: Monaco.editor.ITextModel;
  /** 正在挂载该模型的编辑器数量。 */
  references: number;
  /** 最后一个编辑器卸载后的延迟释放任务。 */
  disposalTimer: ReturnType<typeof setTimeout> | null;
};

/** ArgusFlow 创建且仍存活的 Monaco 模型注册表。 */
const modelEntries = new Map<string, ModelEntry>();

/** 获取或创建指定 URI 的受控模型。 */
export function acquireMonacoModel(
  monaco: MonacoApi,
  modelUri: string,
  language: string,
  initialValue: string,
): Monaco.editor.ITextModel {
  const existing = modelEntries.get(modelUri);
  if (existing) {
    if (existing.disposalTimer !== null) {
      clearTimeout(existing.disposalTimer);
      existing.disposalTimer = null;
    }
    existing.references += 1;
    if (existing.model.getLanguageId() !== language) {
      monaco.editor.setModelLanguage(existing.model, language);
    }
    return existing.model;
  }

  const resource = monaco.Uri.parse(modelUri);
  const model = monaco.editor.getModel(resource)
    ?? monaco.editor.createModel(initialValue, language, resource);
  if (model.getLanguageId() !== language) {
    monaco.editor.setModelLanguage(model, language);
  }
  modelEntries.set(modelUri, {
    model,
    references: 1,
    disposalTimer: null,
  });
  return model;
}

/**
 * 释放模型引用。
 *
 * 零延迟任务晚于 React 同一轮布局切换中的新挂载，因而抽屉切换不会重建模型。
 */
export function releaseMonacoModel(modelUri: string): void {
  const entry = modelEntries.get(modelUri);
  if (!entry) {
    return;
  }
  entry.references -= 1;
  if (entry.references > 0 || entry.disposalTimer !== null) {
    return;
  }
  entry.disposalTimer = setTimeout(() => {
    const current = modelEntries.get(modelUri);
    if (!current || current.references > 0) {
      return;
    }
    current.model.dispose();
    modelEntries.delete(modelUri);
  }, 0);
}
