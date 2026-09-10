import { expect, it } from 'vitest';
import actionSource from '../../../crates/argusflow-browser/src/page/aql/action.js?raw';
import snapshotSource from '../../../crates/argusflow-browser/src/page/aql/snapshot.js?raw';

it('DOM 插入前收起已有选区且不改写内容', () => {
  const action = window.eval(`(${actionSource})`) as (this: Node, action: string) => boolean;
  const input = document.createElement('input');
  input.value = '已有😀文字';
  document.body.append(input);
  input.focus();
  input.setSelectionRange(0, 4);
  expect(action.call(input, 'focus')).toBe(true);
  expect(input.value).toBe('已有😀文字');
  expect(input.selectionStart).toBe(4);
  expect(input.selectionEnd).toBe(4);
  input.readOnly = true;
  expect(action.call(input, 'focus')).toBe(false);
  input.remove();
});

it('属性脚本保持 DOM 先序且不穿透开放的 Shadow Root', () => {
  const snapshot = window.eval(`(${snapshotSource})`) as (this: Node, selectors: string[]) => { path: string; text: string; css: string[] }[];
  const host = document.createElement('div');
  host.innerHTML = '<button>外部</button><custom-panel></custom-panel>';
  host.querySelector('custom-panel')!.attachShadow({ mode: 'open' }).innerHTML = '<button>内部</button>';
  document.body.append(host);
  // jsdom 不提供文字 Range 布局，测试替身只返回确定性几何。
  const original = Range.prototype.getBoundingClientRect;
  Range.prototype.getBoundingClientRect = () => new DOMRect(0, 0, 20, 10);
  try {
    const rows = snapshot.call(host, ['button']);
    expect(rows.map((row) => row.path)).toEqual(['', '/0', '/0/0', '/1']);
    expect(rows.filter((row) => row.css.includes('button'))).toHaveLength(1);
    expect(rows.every((row) => row.text !== '内部')).toBe(true);
  } finally { Range.prototype.getBoundingClientRect = original; host.remove(); }
});
