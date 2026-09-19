/** 独立 Node 驱动：仅浏览器操作使用 Patchright。 */
import { chromium } from 'patchright';
import { connect } from 'node:net';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { scrapeBaidu } from './scrape.mjs';
import { launchOptions } from './launch-options.mjs';

const [pipeName, executablePath, profile, marker] = process.argv.slice(2);
const socket = connect(pipeName);
await once(socket, 'connect');
const lines = createInterface({ input: socket });
const requests = lines[Symbol.asyncIterator]();
/** 整个驱动的硬截止时间，Rust 还会通过 Application 回收完整进程树。 */
const deadline = setTimeout(() => process.exit(1), 100_000);
let context;
socket.on('error', () => process.exit(1));
try {
  context = await chromium.launchPersistentContext(profile, launchOptions(executablePath));
  const page = context.pages()[0] ?? await context.newPage();
  await page.setContent(`<title>${marker}</title><h1>ArgusFlow Patchright Demo</h1>`);
  socket.write(JSON.stringify({ type: 'ready' }) + '\n');
  const request = await requests.next();
  if (request.done || JSON.parse(request.value).type !== 'scrape') {
    throw new Error('Rust 未发送 scrape 请求');
  }
  const report = await scrapeBaidu(page);
  await context.close();
  context = undefined;
  socket.end(JSON.stringify({ type: 'result', report }) + '\n');
} catch (error) {
  socket.end(JSON.stringify({ type: 'error', message: String(error) }) + '\n');
  process.exitCode = 1;
} finally {
  if (context) await context.close();
  clearTimeout(deadline);
  lines.close();
}
