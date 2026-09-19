/** 无任务知识的数据压缩与 Windows VK 解码；保留原始证据引用。 */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const dir=resolve(process.argv[2]);
const evidence=JSON.parse(await readFile(resolve(dir,'model-input/evidence.json'),'utf8'));
const names=new Map([[13,'Enter'],[16,'Shift'],[17,'Control'],[18,'Alt'],[27,'Escape'],[35,'End'],[36,'Home'],[37,'Left'],[38,'Up'],[39,'Right'],[40,'Down'],[162,'LeftControl'],[163,'RightControl'],[160,'LeftShift'],[161,'RightShift'],[164,'LeftAlt'],[165,'RightAlt']]);
const held=new Set();
const keys=[];
for(const e of evidence.events){
 if(e.kind==='Context'){held.clear();keys.push(e);continue;}
 const k=e.kind?.Key;
 if(!k){keys.push(e);continue;}
 const name=names.get(k.vk)??(k.vk>=65&&k.vk<=90?String.fromCharCode(k.vk):`VK_${k.vk}`);
 if(k.down){held.add(name);keys.push({id:e.id,epoch_ms:e.epoch_ms,window:e.window,kind:'key_down',key:name,held:[...held]});}
 else held.delete(name);
}
evidence.events=keys;
for(const s of evidence.samples){
 if(s.focus){
  const target=s.focus.target;
  s.focus.target={name:target.name,class_name:target.class_name,role:target.role,focused:target.focused,bounds:target.bounds};
  if(s.window.class==='Chrome_WidgetWin_1')s.focus.text={omitted:'浏览器原生 TextPattern 与 CDP 同源重复；使用 cdp 字段的 DOM 选区证据'};
 }
 s.ocr=s.ocr.map(b=>({text:b.text,rect:b.rect,confidence:Math.round(b.confidence*1000)/1000}));
}
// 时间仍保持绝对值，事件和样本编号均不改变，未按预设业务步骤分类。
await writeFile(resolve(dir,'model-input/normalized.json'),JSON.stringify(evidence,null,2));
console.log(JSON.stringify({events:keys.length,samples:evidence.samples.length,cdp:evidence.cdp.length,characters:JSON.stringify(evidence).length}));
