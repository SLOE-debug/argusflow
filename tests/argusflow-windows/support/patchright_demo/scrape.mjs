/** 抓取百度首页热搜列表；空结果或页面结构改变时明确失败。 */
export async function scrapeBaidu(page) {
  await page.goto('https://www.baidu.com/', { waitUntil: 'domcontentloaded', timeout: 30_000 });
  const links = page.locator('#hotsearch-content-wrapper li a');
  await links.first().waitFor({ state: 'visible', timeout: 20_000 });
  const items = await links.evaluateAll(elements => elements.map(element => ({
    title: (element.querySelector('.title-content-title')?.textContent ?? element.textContent ?? '').trim(),
    url: element.href,
  })).filter(item => item.title && /^https?:\/\//.test(item.url)).slice(0, 10));
  if (items.length === 0) throw new Error('百度没有返回可用的热搜条目');
  return {
    page_title: await page.title(), page_url: page.url(),
    captured_at: new Date().toISOString(), items,
  };
}
