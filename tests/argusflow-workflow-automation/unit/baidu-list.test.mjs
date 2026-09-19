import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';

const source = readFileSync(new URL('../support/replay/baidu-list.js', import.meta.url), 'utf8');
function page(ranks = [0, 5, 1, 6, 2, 7, 3, 8, 4, 9]) {
  const rows = ranks.map(rank => `<li class="hotsearch-item variable" data-index="${rank}"><a class="title-content variable" href="/s?wd=${encodeURIComponent(`实时标题${rank}`)}"><span class="title-content-title">实时标题${rank}</span></a></li>`).join('');
  const dom = new JSDOM(`<ul id="hotsearch-content-wrapper">${rows}</ul>`, { url: 'https://www.baidu.com/', runScripts: 'outside-only' });
  dom.window.HTMLElement.prototype.getBoundingClientRect = () => ({width:100,height:20});
  return dom;
}
test('两列交错 DOM 按排名取真实标题，排除置顶项，不依赖附加 class', () => {
  const dom = page();
  try {
    const result = dom.window.eval(source);
    assert.deepEqual(Array.from(result.items.slice(0, 3), item => item.rank), [1, 2, 3]);
    for (const item of result.items) assert.equal(dom.window.document.querySelector(item.selector).textContent, item.title);
  } finally { dom.window.close(); }
});
test('重复排名及缺失前三名拒绝猜测', () => {
  for (const ranks of [[1,1,2,3], [1,3,4]]) {
    const dom = page(ranks);
    try { assert.throws(() => dom.window.eval(source), /BAIDU_(RANK_DUPLICATED|TOP_THREE_MISSING)/); }
    finally { dom.window.close(); }
  }
});
test('链接内容和标题不一致时拒绝复制', () => {
  const dom = page();
  try {
    dom.window.document.querySelector('a').href = '/s?wd=其他内容';
    assert.throws(() => dom.window.eval(source), /BAIDU_ROW_INVALID/);
  } finally { dom.window.close(); }
});
