/** 真实百度首页的只读语义列表快照。 */
(() => {
  if (location.href === 'about:blank') return null;
  if (location.origin !== 'https://www.baidu.com') throw new Error('BAIDU_ORIGIN_CHANGED');
  const rootSelector = '#hotsearch-content-wrapper';
  const roots = document.querySelectorAll(rootSelector);
  if (roots.length === 0) return null;
  if (roots.length !== 1 || roots[0].tagName !== 'UL') throw new Error('BAIDU_LIST_AMBIGUOUS');
  const root = roots[0], seen = new Set();
  const items = [...root.querySelectorAll(':scope > li[data-index]')].map(row => {
    const rawRank = row.getAttribute('data-index');
    if (!/^\d+$/.test(rawRank)) throw new Error('BAIDU_RANK_INVALID');
    const rank = Number(rawRank);
    if (seen.has(rank)) throw new Error('BAIDU_RANK_DUPLICATED');
    seen.add(rank);
    const links = row.querySelectorAll(':scope > a.title-content');
    if (links.length !== 1) throw new Error('BAIDU_LINK_AMBIGUOUS');
    const titles = links[0].querySelectorAll(':scope > .title-content-title');
    if (titles.length !== 1) throw new Error('BAIDU_TITLE_AMBIGUOUS');
    const element = titles[0], title = element.textContent;
    const rect = element.getBoundingClientRect(), style = getComputedStyle(element);
    const url = new URL(links[0].href);
    if (!title || title !== title.trim() || /[\r\n]/.test(title) || [...title].length > 80 ||
        rect.width <= 0 || rect.height <= 0 || style.visibility !== 'visible' ||
        url.origin !== location.origin || url.pathname !== '/s' || url.searchParams.get('wd') !== title)
      throw new Error('BAIDU_ROW_INVALID');
    return { rank, title, url: url.href, selector: `${rootSelector} > li[data-index="${rank}"] > a.title-content > .title-content-title`,
      row_class: row.className, title_class: element.className };
  }).filter(item => item.rank > 0).sort((a,b) => a.rank - b.rank);
  if (items.length < 3) return null;
  if (items.slice(0,3).some((item,index) => item.rank !== index + 1)) throw new Error('BAIDU_TOP_THREE_MISSING');
  return { url: location.href, captured_at: new Date().toISOString(), root_selector: rootSelector,
    list_selector: `${rootSelector} > li[data-index] > a.title-content > .title-content-title`,
    root_class: root.className, items };
})()
