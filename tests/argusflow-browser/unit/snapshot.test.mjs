import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {JSDOM} from 'jsdom';
const source=readFileSync(new URL('../../../crates/argusflow-browser/src/page/aql/snapshot.js',import.meta.url),'utf8');
test('不查询 value 时不读取大型隐藏表单；显式查询保留原值而不截断',()=>{
  const dom=new JSDOM('<input type="hidden" id="payload"><span class="title">热搜</span>',{runScripts:'outside-only'});
  const input=dom.window.document.querySelector('input');
  dom.window.Range.prototype.getBoundingClientRect=()=>({x:0,y:0,width:0,height:0,right:0,bottom:0});
  input.value='x'.repeat(224722);
  const snapshot=dom.window.eval(`(${source})`);
  const plain=snapshot.call(dom.window.document,['.title'],false);
  assert.equal(plain.find(v=>v.attributes.id==='payload').value,null);
  const explicit=snapshot.call(dom.window.document,[],true);
  assert.equal(explicit.find(v=>v.attributes.id==='payload').value.length,224722);
  dom.window.close();
});
