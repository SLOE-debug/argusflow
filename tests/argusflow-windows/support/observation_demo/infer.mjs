/** 离线证据推导。不导入示范者，不访问桌面，不接收任务步骤/目标文字/接收人。 */
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { difference } from './frame-diff.mjs';
import { appeared } from './message-change.mjs';
const compact=s=>(s??'').replace(/\s/g,'');
const body=s=>s?.documents?.length===1?s.documents[0].replace(/\r\n/g,'\n'):null;
const vk=(event,key)=>event?.kind?.Key?.vk===key&&event.kind.Key.down;
const control=event=>[17,162,163].some(k=>vk(event,k));
function inRegion(block,sample,region){const [x,y,w,h]=block.rect;const [, ,width,height]=sample.bounds;return x>=width*region[0]&&y>=height*region[1]&&x+w<=width*region[2]&&y+h<=height*region[3];}
const blocks=(s,region)=>(s?.ocr??[]).filter(b=>b.confidence>=.65&&inRegion(b,s,region));
const editor=[.35,.765,.98,.91],messages=[.35,.13,.99,.76],header=[.35,.03,.99,.13];
export function infer({session,events,interactions,samples,browser,finished}){
 const steps=[],unresolved=[];
 const valid=samples.filter(s=>s.image&&s.window&&!s.error).sort((a,b)=>a.from_qpc-b.from_qpc);
 const byId=new Map(events.map(e=>[e.sequence,e]));
 const time=q=>session.epoch_ms+(q-session.qpc)/session.frequency*1000;
 const observations=(a)=>valid.filter(s=>s.window.handle===a.window.handle);
 const before=(a)=>observations(a).filter(s=>s.through_qpc<a.from_qpc).at(-1);
 const after=(a,span=5000)=>observations(a).filter(s=>s.from_qpc>a.through_qpc&&time(s.from_qpc)-time(a.through_qpc)<span);
 const fail=(a,reason)=>unresolved.push({raw:a?.raw??[],reason});
 const evidence=(a,b,c)=>({raw:a.raw,before:b?.image??null,after:c?.image??null,before_sample:b?.id??null,after_sample:c?.id??null,changed_pixels:c?.changed_pixels??null});
 if(!finished?.complete||finished.lost!==0)unresolved.push({reason:'录制未完整结束或原始事件丢失'});
 for(const event of browser.filter(e=>e.type==='copy')){
  const prior=browser.filter(e=>e.url===event.url&&e.epoch_ms<=event.epoch_ms);
  const up=prior.filter(e=>e.type==='pointerup').at(-1),down=prior.filter(e=>e.type==='pointerdown'&&e.epoch_ms<(up?.epoch_ms??0)).at(-1);
  const frames=valid.filter(s=>s.window.class==='Chrome_WidgetWin_1');
  const b=frames.filter(s=>time(s.through_qpc)<(down?.epoch_ms??event.epoch_ms)).at(-1);
  const c=frames.find(s=>time(s.from_qpc)>event.epoch_ms&&time(s.from_qpc)<event.epoch_ms+3000);
  if(!event.selection||!event.xpath||!down||!up||Math.abs(up.x-down.x)<8||!b||!c){unresolved.push({browser_event:event,reason:'复制缺少完整拖选/选区/前后帧证据'});continue;}
  if(compact(event.selection)!==compact(event.node_text)){unresolved.push({browser_event:event,reason:'仅选择了节点的一部分，当前执行器不泛化为整条复制'});continue;}
  steps.push({action:'browser_copy',url:event.url,selector:'xpath='+event.xpath,index:0,observed_text:event.selection,at:event.epoch_ms,evidence:{browser_event:event,before:b.image,after:c.image,selection_drag:[down,up]}});
 }
 let pending=null;
 for(const a of interactions){
  const raw=a.raw.map(id=>byId.get(id));
  const ctrl=raw.some(control),copy=ctrl&&raw.some(e=>vk(e,67)),paste=ctrl&&raw.some(e=>vk(e,86)),enter=!ctrl&&raw.some(e=>vk(e,13));
  if(!copy&&!paste&&!enter)continue;
  const b=before(a),following=after(a),c=following[0];
  if(!b||!c){if(copy||paste)fail(a,'输入缺少同一窗口的真实前后帧');continue;}
  if(copy&&b.window.class==='Notepad'){
   const changed=following.find(s=>s.clipboard_sequence!==b.clipboard_sequence);
   const text=changed?.clipboard,lines=body(changed)?.split('\n');
   const indices=lines?.map((v,i)=>v===text?i:-1).filter(i=>i>=0)??[];
   if(!text||indices.length!==1){fail(a,'复制结果无法唯一对应到记事本中的一行');continue;}
   steps.push({action:'document_copy',line:indices[0],observed_text:text,at:time(a.through_qpc),evidence:evidence(a,b,changed)});
  }else if(paste&&b.window.class==='Notepad'){
   const prior=body(b),text=b.clipboard;
   const changed=following.find(s=>body(s)!==prior&&body(s)!==null);
   if(prior===null||!text||!changed||body(changed).trimEnd()!==(prior+text).trimEnd()){fail(a,'粘贴不能由剪贴板和文档增量共同确认');continue;}
   const saved=interactions.find(s=>s.from_qpc>a.through_qpc&&s.window.handle===a.window.handle&&time(s.from_qpc)-time(a.through_qpc)<8000&&s.raw.some(id=>vk(byId.get(id),83))&&s.raw.some(id=>control(byId.get(id))));
   if(!saved){fail(a,'未观察到随后保存动作');continue;}
   steps.push({action:'document_paste',line:prior===''?0:prior.replace(/\n$/,'').split('\n').length,observed_text:text,at:time(a.through_qpc),evidence:{...evidence(a,b,changed),save_raw:saved.raw}});
  }else if(paste&&b.window.title==='微信'){
   const text=b.clipboard,draft=following.find(s=>blocks(s,editor).some(v=>compact(v.text)===compact(text)));
   if(!text||!draft){fail(a,'聊天粘贴缺少可确认的草稿');continue;}
   const names=blocks(draft,header).map(v=>v.text.trim()).filter(Boolean);
   if(names.length!==1){fail(a,'无法从会话标题唯一推断接收人');continue;}
   pending={a,b,draft,text,recipient:names[0]};
  }else if(enter&&pending&&a.window.handle===pending.a.window.handle){
   const c=following.find(s=>s.editor_empty===true&&appeared(blocks(pending.b,messages),blocks(s,messages),pending.text));
   if(!c){fail(a,'Enter后的编辑区清空和正文新增未确认');pending=null;continue;}
   steps.push({action:'chat_paste_send',recipient:pending.recipient,observed_text:pending.text,at:time(pending.a.through_qpc),evidence:{...evidence(a,pending.draft,c),paste_raw:pending.a.raw,confirmation:'本地界面出现同文且草稿消失；不表示服务器送达'}});
   pending=null;
  }
 }
 if(pending)fail(pending.a,'粘贴后未观察到可确认的发送');
 steps.sort((a,b)=>a.at-b.at);
 let source=null;
 for(let i=0;i<steps.length;i++){
  const s=steps[i];s.id=i;
  if(['browser_copy','document_copy'].includes(s.action)){
   if(s.action==='document_copy'){
    const written=steps.slice(0,i).filter(v=>v.action==='document_paste'&&v.line===s.line&&v.observed_text===s.observed_text).at(-1);
    if(written)s.input_from=written.id;else s.external_document=true;
   }
   source=s;
  }
  else if(!source||compact(source.observed_text)!==compact(s.observed_text))unresolved.push({step:i,reason:'粘贴内容没有与此前复制匹配的数据来源'});
  else{s.input_from=source.id;source=null;}
 }
 return {format:'observation-demo-v1',inference:'offline evidence rules; no demonstration plan input',steps,unresolved,limits:['仅完整节点拖选、逐行文档复制和当前双栏聊天布局有执行器','窗口出现不等于已经推断启动方式；回放创建新的工作窗口','保留观察到的顺序，不从三次样本擅自推断循环或泛化到任意列表']};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const directory=resolve(process.argv[2]);
 const json=name=>JSON.parse(readFileSync(join(directory,name),'utf8'));
 const lines=name=>readFileSync(join(directory,name),'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
 const result=infer({session:json('session.json'),finished:json('finished.json'),events:lines('events.jsonl'),interactions:lines('interactions.jsonl'),samples:lines('samples.jsonl'),browser:lines('browser.jsonl')});
 for(const step of result.steps){
  const e=step.evidence;e.pixel_difference=difference(directory,e.before,e.after);
  if(!e.pixel_difference.comparable||(step.action!=='document_copy'&&e.pixel_difference.changed_pixels===0))result.unresolved.push({step:step.id,reason:'实际操作前后帧没有可确认的像素变化'});
 }
 writeFileSync(join(directory,'inferred-workflow.json'),JSON.stringify(result,null,2));
 console.log(JSON.stringify({steps:result.steps.map(s=>({id:s.id,action:s.action,input_from:s.input_from,recipient:s.recipient})),unresolved:result.unresolved},null,2));
}
