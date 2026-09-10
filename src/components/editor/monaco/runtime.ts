import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
// 核心 API 不装配交互功能；语言 provider 需要对应 contribution 才能显示。
import 'monaco-editor/esm/vs/editor/contrib/hover/browser/hoverContribution.js';
import 'monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js';
import 'monaco-editor/esm/vs/editor/contrib/snippet/browser/snippetController2.js';
import 'monaco-editor/esm/vs/editor/contrib/format/browser/formatActions.js';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker.js?worker';

/** 编辑器 worker 从本站打包资源加载，不依赖 CDN。 */
self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
export { monaco };
