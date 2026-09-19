/** 演示编排；执行计划单独保存，不能作为模型输入。 */
import {mkdir,writeFile,access} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const root=process.cwd();
const directory=resolve(process.argv[2]);
await mkdir(directory,{recursive:true});
const page=resolve(directory,'source.html');
await writeFile(page,`<!doctype html><meta charset="utf-8"><title>ArgusFlow Evidence</title>
<style>body{font:28px 'Microsoft YaHei';padding:80px;background:#f4f7fb}li{margin:38px 0}span{background:white;padding:0}</style>
<h1>资料清单</h1><ul><li><span>整理项目会议记录</span></li><li><span>核对本周采购清单</span></li><li><span>归档已完成的测试结果</span></li></ul>`);
const steps=[0,1,2].flatMap(index=>[{action:'browser_copy',url:pathToFileURL(page).href,selector:'li span',index},{action:'document_paste'}]);
steps.push({action:'document_copy',line:1},{action:'document_copy',line:0});
await writeFile(resolve(directory,'actor-private.json'),JSON.stringify({format:'observation-demo-v1',unresolved:[],steps},null,2));
const executable=resolve(root,'target/debug/examples/observation_demo.exe');
function start(args){const child=spawn(executable,args,{cwd:root,env:{...process.env,ARGUSFLOW_ISOLATED_RECORDING:'1'},windowsHide:true,stdio:['ignore','pipe','pipe']});child.stdout.pipe(process.stdout);child.stderr.pipe(process.stderr);return {child,done:new Promise((yes,no)=>{child.on('error',no);child.on('exit',code=>yes(code));})};}
const observer=start(['observe',directory,'180']);
try {
 let ready=false;
 for(let n=0;n<200;n++){try{await access(resolve(directory,'session.json'));ready=true;break;}catch{}await new Promise(r=>setTimeout(r,100));}
 if(!ready)throw Error('Recorder did not become ready');
 const actor=start(['replay',directory,resolve(directory,'actor-private.json')]);
 const code=await actor.done;
 await new Promise(r=>setTimeout(r,1800));
 if(code!==0)throw Error(`Actor failed: ${code}`);
}finally{await writeFile(resolve(directory,'stop'),'');const code=await observer.done;if(code!==0)throw Error(`Recorder failed: ${code}`);}
