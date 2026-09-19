/** 示范端操作浏览器；动作命令不写入证据，DOM 观察器只看页面事件。 */
import { chromium } from 'patchright';
import { connect } from 'node:net';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { observe } from './dom-observer.mjs';
import { launchOptions } from './launch-options.mjs';
import { observeCdp } from './cdp-observer.mjs';
const [pipeName,executablePath,profile,marker,journal]=process.argv.slice(2);
const socket=connect(pipeName); await once(socket,'connect');
const lines=createInterface({input:socket});
let context;
let stopCdp;
const timer=setTimeout(()=>process.exit(1),600000);
try {
 context=await chromium.launchPersistentContext(profile,launchOptions(executablePath));
 await observe(context,journal);
 const page=context.pages()[0]??await context.newPage();
 stopCdp=await observeCdp(context,page,journal.replace(/browser\.jsonl$/, 'cdp.jsonl'));
 await page.goto('about:blank');
 socket.write(JSON.stringify({type:'ready'})+'\n');
 for await (const line of lines) {
  const r=JSON.parse(line);
  if(r.type==='open') {await page.goto(r.url,{waitUntil:'domcontentloaded'});const cdp=await context.newCDPSession(page);const {windowId}=await cdp.send('Browser.getWindowForTarget');await cdp.send('Browser.setWindowBounds',{windowId,bounds:{windowState:'maximized'}});await cdp.detach();await page.bringToFront();}
  else if(r.type==='copy') {
   await page.bringToFront();
   const node=page.locator(r.selector).nth(r.index??0);
   await node.waitFor({state:'visible'}); await node.scrollIntoViewIfNeeded();
   const rect=await node.boundingBox();if(!rect)throw Error('选区目标没有可见边界');
   await page.waitForTimeout(1400);
   // 真正按住鼠标拖过文字，再触发复制；不读取 DOM 文本写入目标应用。
   await page.keyboard.down('Alt');
   await page.mouse.move(rect.x+1,rect.y+rect.height/2);await page.mouse.down();
   await page.mouse.move(rect.x+rect.width-1,rect.y+rect.height/2,{steps:30});await page.mouse.up();await page.keyboard.up('Alt');
   if(!await page.evaluate(()=>getSelection()?.toString()))throw Error('拖选未形成文字选区，停止复制');
   await page.waitForTimeout(1200);await page.keyboard.press('Control+c');await page.waitForTimeout(1500);
  } else if(r.type==='close') {await stopCdp();stopCdp=undefined;await context.close();context=undefined;socket.end(JSON.stringify({type:'closed'})+'\n');break;}
  else throw Error('未知命令');
  socket.write(JSON.stringify({type:'done'})+'\n');
 }
}catch(e){socket.end(JSON.stringify({type:'error',message:String(e)})+'\n');process.exitCode=1;}
finally{if(stopCdp)await stopCdp();if(context)await context.close();lines.close();clearTimeout(timer);}
