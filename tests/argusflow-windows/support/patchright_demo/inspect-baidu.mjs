/** 在独立 Chrome 中观察真实百度 DOM；保留实际标签、属性和父级结构供定位审阅。 */
import { chromium } from 'patchright';
import { launchOptions } from './launch-options.mjs';
import { mkdir, writeFile, access } from 'node:fs/promises';
import path from 'node:path';

const output = path.resolve(process.argv[2]);
await mkdir(output, { recursive: false });
let executable;
for (const root of [process.env.LOCALAPPDATA, process.env.ProgramFiles, process.env['ProgramFiles(x86)']].filter(Boolean)) {
  const candidate = path.join(root, 'Google/Chrome/Application/chrome.exe');
  try { await access(candidate); executable = candidate; break; } catch {}
}
if (!executable) throw new Error('没有找到已安装 Chrome');
const browser = await chromium.launch(launchOptions(executable));
try {
  const page = await browser.newPage({ viewport: null });
  const snapshots = [];
  for (let attempt = 0; attempt < 3; attempt++) {
    await page.goto('https://www.baidu.com/', { waitUntil: 'domcontentloaded', timeout: 30000 });
    await page.locator('#hotsearch-content-wrapper li a').first().waitFor({ state: 'visible', timeout: 20000 });
    const snapshot = await page.evaluate(() => {
      const root = document.querySelector('#hotsearch-content-wrapper');
      const describe = e => ({ tag: e.tagName, id: e.id, class: e.className, role: e.getAttribute('role'), attributes: Object.fromEntries([...e.attributes].map(a => [a.name, a.value])) });
      return { url: location.href, title: document.title, at: new Date().toISOString(), rootCount: document.querySelectorAll('#hotsearch-content-wrapper').length,
        root: describe(root), parent: describe(root.parentElement), html: root.outerHTML,
        rows: [...root.querySelectorAll('li')].map(e => ({ ...describe(e), text: e.innerText, links: [...e.querySelectorAll('a')].map(a => ({ ...describe(a), href: a.href, html: a.innerHTML })) })) };
    });
    snapshots.push(snapshot);
    console.log(JSON.stringify({attempt, url:snapshot.url, root:snapshot.root, rows:snapshot.rows}));
  }
  await writeFile(path.join(output, 'dom-inspection.json'), JSON.stringify(snapshots, null, 2));
} finally { await browser.close(); }
