// @vitest-environment node
import { readFileSync } from 'node:fs';
import { beforeAll, expect, it } from 'vitest';
import { initSync, inspect, completions, hover, importEnglish } from '../../../src/features/aql/generated/aql';
import { exportSource, readAqlFile } from '../../../src/features/aql/files';
import type { DocumentAnalysis, LanguageService } from '../../../src/features/aql/contracts';
import { completionContent, hoverContent } from '../../../src/components/editor/monaco/presentation';

beforeAll(() => initSync({ module: readFileSync(new URL('../../../src/features/aql/generated/aql_bg.wasm', import.meta.url)) }));
const service: LanguageService = {
  inspect,
  completions: (source, position) => completions(source, position.line, position.column),
  hover: (source, position) => hover(source, position.line, position.column),
  importEnglish,
};

it('真实 WASM 保护参数、正则、中文字符串和 CSS，格式化可重复', () => {
  const selector = JSON.stringify('iframe[title="按钮"]');
  const source = `/*窗口*/ 框架(css(${selector})) >> 输入框(名称 匹配 /中[文/]😀/i 或 名称 = $名称)`;
  const analysis: DocumentAnalysis = inspect(source);
  expect(analysis.diagnostics).toEqual([]);
  expect(analysis.english).toContain('frame(css("iframe[title=\\"按钮\\"]"))');
  expect(analysis.english).toContain('/中[文/]😀/i or name = $名称');
  expect(inspect(analysis.formatted!).formatted).toBe(analysis.formatted);
  expect(inspect(importEnglish(analysis.english!)).english).toBe(analysis.english);
});

it('WASM 补全、悬浮和诊断使用中文 UTF-16 范围', () => {
  const source = '// 😀\r\n按';
  const button = service.completions(source, { line: 1, column: 1 }).find((item) => item.label === '按钮');
  expect(button?.range).toEqual({ start: { line: 1, column: 0 }, end: { line: 1, column: 1 } });
  expect(button?.insert_text).toBe('按钮($0)');
  expect(service.hover('按钮()', { line: 0, column: 1 })?.title).toContain('button');
  expect(service.completions('按钮(名称 = "按', { line: 0, column: 10 })).toEqual([]);
  const invalid = inspect('// 😀\r\n按钮(名称 = "😀中文", 可用 = )');
  expect(invalid.english).toBeUndefined();
  expect(invalid.formatted).toBeUndefined();
  expect(invalid.diagnostics[0].range).toEqual({ start: { line: 1, column: 21 }, end: { line: 1, column: 22 } });
});

it('英文输入不会被中文标签二次过滤，候选显示说明并使用片段', () => {
  const item = service.completions('but', { line: 0, column: 3 }).find((item) => item.label === '按钮')!;
  const completion = completionContent(item, 5, 4);
  expect(completion.filterText).toBe('button');
  expect(completion.insertText).toBe('按钮($0)');
  expect(completion.insertTextRules).toBe(4);
  expect(completion.detail).toContain('角色');
  expect(completion.documentation).toMatchObject({ value: expect.stringContaining('UIA'), isTrusted: false });
  expect(service.completions('cla', { line: 0, column: 3 }).map((item) => item.label)).toEqual(expect.arrayContaining(['界面.类名', '网页.类名']));
});

it('悬浮展示属性与参数类型、用法和中文示例', () => {
  const item = service.hover('按钮(名称 = $目标)', { line: 0, column: 9 })!;
  const content = hoverContent(item);
  expect(content.contents.map((item) => item.value).join('\n')).toContain('参数 · $目标');
  expect(content.contents.map((item) => item.value).join('\n')).toContain('$目标: 文本');
  expect(content.range).toEqual({ startLineNumber: 1, startColumn: 9, endLineNumber: 1, endColumn: 12 });
  const text = service.hover('文本(文本 = "内容")', { line: 0, column: 4 })!;
  expect(text.kind).toBe('Attribute');
  expect(text.signature).toBe('文本: 文本');
});

it('英文文件导入后只导出当前有效草稿，并拒绝错误文件编码', async () => {
  const source = await readAqlFile(new File(['button(name = "保存")'], 'query.aql'), service);
  expect(source).toBe('按钮(名称 = "保存")');
  const analysis = inspect(source);
  expect(exportSource({ status: 'ready', source, revision: 1, analysis })).toBe('button(name = "保存")');
  expect(exportSource({ status: 'pending', source, revision: 2 })).toBeUndefined();
  await expect(readAqlFile(new File([new Uint8Array([0xff])], 'query.aql'), service)).rejects.toThrow('UTF-8');
  await expect(readAqlFile(new File(['button()'], 'query.txt'), service)).rejects.toThrow('.aql');
  await expect(readAqlFile(new File(['x'.repeat(65_537)], 'query.aql'), service)).rejects.toThrow('64 KiB');
});
