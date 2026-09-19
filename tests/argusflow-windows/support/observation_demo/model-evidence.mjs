/** 只读取录制事实；不读取 actor-private.json 或规则推导的 workflow。 */
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve} from 'node:path';
const dir=resolve(process.argv[2]);
const read=async name=>JSON.parse(await readFile(resolve(dir,name),'utf8'));
const lines=async name=>(await readFile(resolve(dir,name),'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const session=await read('session.json');
const finished=await read('finished.json');
if(!finished.complete || finished.lost)throw Error('Incomplete recorder');
const all=await lines('samples.jsonl');
const samples=all.filter(s=>s.window && !s.error);
if(!samples.length)throw Error('No samples');
const selected=[];
let previous='';
for(const s of samples){
 const key=JSON.stringify([s.window.handle,s.focus?.text,s.clipboard_sequence]);
 if(key!==previous)selected.push(s);
 previous=key;
}
const handles=new Set(samples.map(s=>s.window.handle));
const epoch=qpc=>Math.round(session.epoch_ms+(qpc-session.qpc)*1000/session.frequency);
const events=(await lines('events.jsonl')).filter(e=>handles.has(e.window.handle)).map(e=>({id:e.sequence,epoch_ms:epoch(e.qpc),window:e.window.handle,point:e.point,origin:e.origin,kind:e.kind}));
const cdp=[];
for(const [index,s] of (await lines('cdp.jsonl')).entries()){
 if(s.error || s.observation?.lost)throw Error('Incomplete CDP recording');
 for(const e of s.observation?.events??[]){
  const selection=structuredClone(e.selection);
  for(const k of ['anchor','focus'])if(selection[k]?.parent){const p=selection[k].parent;selection[k].parent={node:p.node,tag:p.tag,id:p.id,bounds:p.bounds};}
  cdp.push({id:`cdp-${index}-${e.sequence}`,epoch_ms:Math.round(s.time_origin+e.time),kind:e.kind,point:e.point,button:e.button,trusted:e.trusted,selection,target:e.target?{node:e.target.node,tag:e.target.tag,id:e.target.id,bounds:e.target.bounds}:null});
 }
}
await mkdir(resolve(dir,'model-input'),{recursive:true});
const frames=[];
const facts=selected.map(s=>{
 const native=s.window.class==='Notepad';
 const b=s.focus?.target.bounds;
 const x=native&&b?Math.max(0,b[0]-s.bounds[0]):0;
 const y=native&&b?Math.max(0,b[1]-s.bounds[1]):110;
 const crop=[x,y,Math.min(1050,s.bounds[2]-x),Math.min(native?360:650,s.bounds[3]-y)];
 const ocr=s.ocr.filter(b=>b.rect[0]>=x&&b.rect[1]>=y&&b.rect[0]+b.rect[2]<=x+crop[2]&&b.rect[1]+b.rect[3]<=y+crop[3]);
 const output=`model-input/frame-${s.id}.png`;
 frames.push({id:s.id,input:s.image,output,crop});
 return {id:`sample-${s.id}`,from_epoch_ms:epoch(s.from_qpc),frame_through_epoch_ms:epoch(s.frame_through_qpc),through_epoch_ms:epoch(s.through_qpc),window:s.window,focus:s.focus?{runtime_id:s.focus.runtime_id,target:s.focus.target,text:s.focus.text}:null,clipboard:s.clipboard_observation,clipboard_current:s.clipboard,ocr,image:output,crop};
});
const evidence={schema:'observed-evidence-v1',notes:['只有录制事实，不包含动作计划。时间为毫秒，采样并非原子快照。','UIA 文本偏移是 UTF-16，CR 为一个单位。CDP 坐标为页面 CSS 像素，原生鼠标为桌面物理像素。','OCR 坐标是窗口内物理像素；图片已裁去应用标签栏，crop 是原图裁剪范围。','CDP 注入的浏览器鼠标不一定经过 Windows 低级输入钩子；CDP 事件是浏览器动作证据。','相同文本/剪贴板序号不单独证明复制或粘贴。未知结论必须保留不确定性。'],recording:finished,errors:all.filter(s=>s.error),samples:facts,events,cdp};
await writeFile(resolve(dir,'model-input/evidence.json'),JSON.stringify(evidence,null,2));
// 每个不同文档/非折叠选区的首帧，最多 12 张；不按预设步骤挑图。
const seen=new Set();
const chosen=[];
for(let i=0;i<selected.length;i++){
 const s=selected[i],t=s.focus?.text?.Available;
 const key=JSON.stringify([s.window.class,t?.document,t?.selections?.filter(v=>!v.collapsed)]);
 if(!seen.has(key)){seen.add(key);chosen.push(frames[i]);}
}
const bounded=chosen.length<=12?chosen:Array.from({length:12},(_,i)=>chosen[Math.round(i*(chosen.length-1)/11)]);
await writeFile(resolve(dir,'model-input/frames.json'),JSON.stringify(bounded,null,2));
console.log(JSON.stringify({samples:all.length,retained:facts.length,events:events.length,cdp:cdp.length,frames:Math.min(12,chosen.length),errors:evidence.errors.length}));
