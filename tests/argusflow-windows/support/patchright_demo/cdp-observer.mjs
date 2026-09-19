/** 通过真实 CDP 读取基础服务的只读观察脚本；不接收执行计划。 */
import { readFileSync, appendFileSync } from 'node:fs';
const source = readFileSync(new URL('../../../../crates/argusflow-browser/src/page/observation.js', import.meta.url), 'utf8');
export async function observeCdp(context, page, journal) {
  const session = await context.newCDPSession(page);
  await session.send('Page.enable');
  await session.send('Page.addScriptToEvaluateOnNewDocument', {source});
  await session.send('Runtime.evaluate', {expression: source});
  let stopped = false;
  let pending = Promise.resolve();
  async function sample() {
    const from = Date.now();
    try {
      const result = await session.send('Runtime.evaluate', {
        expression: 'JSON.stringify({url:location.href,title:document.title,time_origin:performance.timeOrigin,observation:globalThis.__argusflowRecorderV1?.take()})',
        returnByValue: true,
      });
      if (result.exceptionDetails) throw Error('CDP observation evaluation failed');
      appendFileSync(journal, JSON.stringify({from_epoch_ms:from,through_epoch_ms:Date.now(),...JSON.parse(result.result.value)})+'\n');
    } catch (error) {
      if (!stopped) appendFileSync(journal,JSON.stringify({from_epoch_ms:from,error:String(error)})+'\n');
    }
  }
  const timer=setInterval(()=> {pending=pending.then(sample);}, 250);
  return async()=>{clearInterval(timer);await pending;await sample();stopped=true;await session.detach();};
}
