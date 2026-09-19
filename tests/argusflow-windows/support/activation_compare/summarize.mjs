/** 只汇总实际 worker 记录，不把 API 返回 true 直接计为成功。 */
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
const directory=resolve(process.argv[2]);
const trials=JSON.parse(await readFile(resolve(directory,"results.json"),"utf8"));
const groups=new Map();
for(const trial of trials){
  const key=`${trial.restriction}/${trial.strategy}`;
  if(!groups.has(key))groups.set(key,[]);
  groups.get(key).push(trial);
}
const median=values=>{
  const sorted=values.toSorted((a,b)=>a-b);
  if(!sorted.length)return null;
  const mid=Math.floor(sorted.length/2);
  return Number((sorted.length%2?sorted[mid]:(sorted[mid-1]+sorted[mid])/2).toFixed(2));
};
const success=t=>Boolean(t.attempt?.foreground_stable&&!t.attempt.error&&!t.error&&!t.worker_timeout&&t.attempt.after.foreground===t.attempt.target);
const rows=[...groups].map(([key,items])=>{
  const passed=items.filter(success);
  return {
    group:key,passed:passed.length,total:items.length,
    median_api_ms:median(passed.map(t=>t.attempt.api_ms)),
    median_elapsed_ms:median(passed.map(t=>t.attempt.elapsed_ms)),
    timeouts:items.filter(t=>t.worker_timeout).length,
    alt_stuck:items.filter(t=>t.attempt?.after.alt_down).length,
    menu_mode_after:items.filter(t=>t.attempt&&(t.attempt.after.gui_flags&4)!==0).length,
    topmost_changed:items.filter(t=>t.attempt&&t.attempt.before.target_topmost!==t.attempt.after.target_topmost).length,
    cases:[...new Set(items.map(t=>t.case))].map(name=>({name,passed:items.filter(t=>t.case===name&&success(t)).length,total:items.filter(t=>t.case===name).length}))
  };
});
await writeFile(resolve(directory,"summary.json"),JSON.stringify(rows,null,2));
console.table(rows.map(({cases,...row})=>row));
for(const row of rows)console.log(row.group,row.cases.map(c=>`${c.name}=${c.passed}/${c.total}`).join(" "));
