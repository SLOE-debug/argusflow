import { expect, it, vi } from "vitest";

it("真实 Monaco 运行时装配鼠标悬浮与自动候选控制器", async () => {
  // jsdom 不实现媒体查询；只装载模块并检查注册表，不创建浏览器或编辑器视图。
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  try {
    const { monaco } =
      await import("../../../src/components/editor/monaco/runtime");
    const { expressionLanguage } =
      await import("../../../src/components/workflow/value-editor/expressionLanguage");
    const model = monaco.editor.createModel("line = 0");
    const language = expressionLanguage("test-workflow-formula", () => []);
    const binding = language.register(monaco, model, () => false);
    try {
      monaco.editor.setModelLanguage(model, language.id);
      expect(model.getLanguageId()).toBe(language.id);
      expect(
        monaco.editor
          .tokenize(model.getValue(), language.id)[0]
          .map((token) => token.type),
      ).toEqual(
        expect.arrayContaining([
          "operator.test-workflow-formula",
          "number.test-workflow-formula",
        ]),
      );
    } finally {
      model.dispose();
      binding.dispose();
    }
    const { EditorExtensionsRegistry } =
      // @ts-expect-error Monaco 内部注册表没有声明文件；此回归验证实际模块装配。
      await import("monaco-editor/esm/vs/editor/browser/editorExtensions.js");
    const contributions: readonly { readonly id: string }[] =
      EditorExtensionsRegistry.getEditorContributions();
    expect(contributions.map((entry) => entry.id)).toEqual(
      expect.arrayContaining([
        "editor.contrib.contentHover",
        "editor.contrib.suggestController",
        "snippetController2",
      ]),
    );
  } finally {
    vi.unstubAllGlobals();
  }
}, 30_000);
