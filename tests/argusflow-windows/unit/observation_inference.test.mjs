import { test } from 'node:test';
import assert from 'node:assert/strict';
import { infer } from '../support/observation_demo/infer.mjs';
import { appeared } from '../support/observation_demo/message-change.mjs';
const key=(id,qpc,vk,window=1)=>({sequence:id,qpc,kind:{Key:{vk,down:true}},window:{handle:window}});
function fixture(){
 const events=[key(1,100,162),key(2,110,67),key(3,300,162,2),key(4,310,86,2),key(5,600,13,2)];
 const sample=(id,qpc,window,extra)=>({id,from_qpc:qpc,through_qpc:qpc+10,image:`${id}.bmp`,window,bounds:[0,0,1000,1000],ocr:[],...extra});
 const chat={handle:2,title:'微信'},document={handle:1,class:'Notepad'};
 return {session:{epoch_ms:0,qpc:0,frequency:1000},finished:{complete:true,lost:0},browser:[],events,
  interactions:[{raw:[1,2],from_qpc:100,through_qpc:110,window:document},{raw:[3,4],from_qpc:300,through_qpc:310,window:chat},{raw:[5],from_qpc:600,through_qpc:600,window:chat}],
  samples:[sample(0,50,document,{documents:['甲\n完全不同的内容\n'],clipboard_sequence:1}),sample(1,150,document,{documents:['甲\n完全不同的内容\n'],clipboard_sequence:2,clipboard:'完全不同的内容'}),
   sample(2,250,chat,{clipboard:'完全不同的内容'}),sample(3,400,chat,{clipboard:'完全不同的内容',ocr:[{text:'任意接收人乙',confidence:.99,rect:[400,40,150,30]},{text:'完全不同的内容',confidence:.99,rect:[400,800,150,30]}]}),
   sample(4,700,chat,{editor_empty:true,ocr:[{text:'任意接收人乙',confidence:.99,rect:[400,40,150,30]},{text:'完全不同的内容',confidence:.99,rect:[700,500,150,30]}]})]};
}
test('从证据推断行号、接收人和跨应用数据来源，不默认示范条目',()=>{
 const result=infer(fixture());assert.deepEqual(result.unresolved,[]);
 assert.equal(result.steps[0].line,1);assert.equal(result.steps[1].recipient,'任意接收人乙');assert.equal(result.steps[1].input_from,0);
});
test('不存在操作前帧时保持未知，不把事后截图冒充前帧',()=>{
 const f=fixture();f.samples.shift();assert.ok(infer(f).unresolved.length);
});
test('Enter后仍有草稿不得推断发送成功',()=>{
 const f=fixture();f.samples.at(-1).editor_empty=false;f.samples.at(-1).ocr.push({text:'完全不同的内容',confidence:.99,rect:[400,800,150,30]});
 const result=infer(f);assert.ok(result.unresolved.length);assert.ok(!result.steps.some(s=>s.action==='chat_paste_send'));
});
test('不同的剪贴板内容不能接到此前的复制数据来源',()=>{
 const f=fixture();f.samples[1].clipboard='甲';f.samples[1].documents=['甲\n完全不同的内容\n'];
 assert.ok(infer(f).unresolved.some(u=>u.reason.includes('数据来源')));
});
test('丢失原始事件不得声明录制完整',()=>{
 const f=fixture();f.finished.lost=1;assert.ok(infer(f).unresolved.length);
});
test('正文已有同文但没有新增，不能用旧消息证明本次发送',()=>{
 const f=fixture();f.samples[2].ocr.push({text:'完全不同的内容',confidence:.99,rect:[700,500,150,30]});
 const result=infer(f);assert.ok(result.unresolved.length);assert.ok(!result.steps.some(s=>s.action==='chat_paste_send'));
});
test('缺少接收人标题时不采用示范者的联系人默认值',()=>{
 const f=fixture();f.samples[3].ocr.shift();const result=infer(f);
 assert.ok(result.unresolved.some(u=>u.reason.includes('接收人')));assert.ok(!result.steps.some(s=>s.recipient));
});
test('空输入框含语音提示时，仍须同时观察到正文新增',()=>{
 const f=fixture();f.samples.at(-1).ocr.push({text:'按住鼠标 语音输入文字',confidence:.99,rect:[400,800,190,30]});
 assert.deepEqual(infer(f).unresolved,[]);
 f.samples.at(-1).ocr=f.samples.at(-1).ocr.filter(b=>b.text!=='完全不同的内容');
 assert.ok(infer(f).unresolved.length);
});
test('同文旧消息滚出顶部时，用两个正文锚点确认底部新增',()=>{
 const b=(text,y)=>({text,rect:[400,y,200,20]});
 const before=[b('重复消息内容',140),b('正文锚点第一条',300),b('正文锚点第二条',450)];
 const after=[b('正文锚点第一条',230),b('正文锚点第二条',380),b('重复消息内容',520)];
 assert.equal(appeared(before,after,'重复消息内容'),true);
 assert.equal(appeared(before,before,'重复消息内容'),false);
 assert.equal(appeared(before,[b('重复消息内容',140),b('正文锚点第一条',230)],'重复消息内容'),false);
 assert.equal(appeared(before,before.map(v=>b(v.text,v.rect[1]-70)),'重复消息内容'),false);
});
