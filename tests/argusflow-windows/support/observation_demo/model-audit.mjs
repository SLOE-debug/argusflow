/** 模型输出的只读审计；不修补模型答案，也不调用任何执行器。 */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const dir=resolve(process.argv[2]);
const read=async name=>JSON.parse(await readFile(resolve(dir,name),'utf8'));
const evidence=await read('model-input/normalized.json');
const ids=new Set([...evidence.samples,...evidence.events,...evidence.cdp].map(e=>String(e.id)));
const control=e=>e.held?.some(k=>k.endsWith('Control'));
const expected={browser_copy:evidence.cdp.filter(e=>e.kind==='copy').length,native_copy:evidence.events.filter(e=>e.key==='C'&&control(e)).length,paste_requests:evidence.events.filter(e=>e.key==='V'&&control(e)).length,save_requests:evidence.events.filter(e=>e.key==='S'&&control(e)).length};
const reports=[];
for(const label of process.argv.slice(3)){
 const result=await read(`${label}-workflow.json`);
 const response=await read(`${label}-response.json`);
 const steps=result.steps??[];
 const native=s=>/notepad/i.test(s.source_app??'');
 const counts={browser_copy:steps.filter(s=>s.action==='copy'&&!native(s)).length,native_copy:steps.filter(s=>s.action==='copy'&&native(s)).length,paste_requests:steps.filter(s=>s.action==='paste').length,save_requests:steps.filter(s=>s.action==='save_request').length};
 const missing=Object.entries(expected).filter(([key,count])=>counts[key]!==count).map(([category,count])=>({category,observed_requests:count,model_steps:counts[category]}));
 const invalidReferences=steps.flatMap((s,index)=>(s.evidence_ids??[]).filter(id=>!ids.has(String(id))).map(id=>({step:index+1,id})));
 const invalidRanges=[];
 for(const [index,s] of steps.entries()){
  if(s.action!=='copy'||native(s)||!s.selection_range||Array.isArray(s.selection_range))continue;
  const copies=evidence.cdp.filter(e=>e.kind==='copy'&&e.selection.text===s.text);
  const r=s.selection_range;
  if(copies.length===1&&r.start!==undefined&&r.end!==undefined){
   const selection=copies[0].selection;
   const a=selection.anchor,f=selection.focus;
   if(a?.node===f?.node&&(Math.min(a.offset,f.offset)!==r.start||Math.max(a.offset,f.offset)!==r.end))invalidRanges.push({step:index+1,claimed:[r.start,r.end],observed:[Math.min(a.offset,f.offset),Math.max(a.offset,f.offset)]});
  }
 }
 reports.push({label,steps:steps.length,expected,counts,missing,invalid_references:invalidReferences,invalid_ranges:invalidRanges,model_replay_ready:result.replay_ready,automatic_execution_allowed:false,usage:response.usage});
}
await writeFile(resolve(dir,'model-audit.json'),JSON.stringify(reports,null,2));
console.log(JSON.stringify(reports.map(r=>({label:r.label,steps:r.steps,counts:r.counts,missing:r.missing,invalid_references:r.invalid_references.length,invalid_ranges:r.invalid_ranges})),null,2));
