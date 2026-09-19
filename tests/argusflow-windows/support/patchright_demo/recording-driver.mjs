/** 录制 demo 的浏览器动作端：每个请求只读取一个热搜条目。 */
import { chromium } from 'patchright';
import { connect } from 'node:net';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { launchOptions } from './launch-options.mjs';

const [pipeName, executablePath, profile, marker] = process.argv.slice(2);
const socket = connect(pipeName);
await once(socket, 'connect');
const lines = createInterface({ input: socket });
let context;
socket.on('error', () => process.exit(1));
const deadline = setTimeout(() => process.exit(1), 600_000);
try {
  context = await chromium.launchPersistentContext(profile, launchOptions(executablePath));
  const page = context.pages()[0] ?? await context.newPage();
  await page.setContent(`<title>${marker}</title><h1>ArgusFlow recording demo</h1>`);
  socket.write(JSON.stringify({ type: 'ready' }) + '\n');
  for await (const line of lines) {
    const request = JSON.parse(line);
    switch (request.type) {
      case 'open':
        await page.goto(request.url, { waitUntil: 'domcontentloaded', timeout: 30_000 });
        // 浏览器继续后台读取；最小化本次专用窗口，为原生编辑器交接前台。
        {
          const session = await context.newCDPSession(page);
          const { windowId } = await session.send('Browser.getWindowForTarget');
          await session.send('Browser.setWindowBounds', { windowId, bounds: { windowState: 'minimized' } });
          await session.detach();
        }
        socket.write(JSON.stringify({ type: 'opened', url: page.url() }) + '\n');
        break;
      case 'read': {
        const item = page.locator(request.selector).nth(request.index);
        await item.waitFor({ state: 'visible', timeout: 20_000 });
        const title = (await item.locator('.title-content-title').innerText()).trim();
        if (!title || title.length > 80 || /[\r\n]/.test(title)) throw new Error('条目不是可用的单行标题');
        socket.write(JSON.stringify({ type: 'item', title, url: await item.getAttribute('href') }) + '\n');
        break;
      }
      case 'close':
        await context.close();
        context = undefined;
        socket.end(JSON.stringify({ type: 'closed' }) + '\n');
        break;
      default: throw new Error('未知浏览器动作');
    }
    if (request.type === 'close') break;
  }
} catch (error) {
  socket.end(JSON.stringify({ type: 'error', message: String(error) }) + '\n');
  process.exitCode = 1;
} finally {
  if (context) await context.close();
  lines.close();
  clearTimeout(deadline);
}
