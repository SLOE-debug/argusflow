/** 同一事实按时间合并，不生成动作标签或补充业务目标。 */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const dir=resolve(process.argv[2]);
const input=JSON.parse(await readFile(resolve(dir,'model-input/normalized.json'),'utf8'));
const {samples,events,cdp,...metadata}=input;
const timeline=[...samples.map(s=>({source:'native_sample',at:s.from_epoch_ms,...s})),...events.map(e=>({source:'input',at:e.epoch_ms,...e})),...cdp.map(e=>({source:'cdp',at:e.epoch_ms,...e}))].sort((a,b)=>a.at-b.at);
await writeFile(resolve(dir,'model-input/timeline.json'),JSON.stringify({...metadata,timeline},null,2));
console.log(JSON.stringify({entries:timeline.length,first:timeline[0].at,last:timeline.at(-1).at}));
