/** 通过共同正文锚点的位移区分新增消息与原有同文；不读取执行计划。 */
const text=b=>b.text.replace(/\s/g,'');
const center=b=>b.rect[1]+b.rect[3]/2;
export function appeared(before,after,expected){
 const target=expected.replace(/\s/g,'');
 const prior=before.filter(b=>text(b)===target),current=after.filter(b=>text(b)===target);
 if(current.length>prior.length)return true;
 const shifts=[];
 for(const block of before){
  if(text(block)===target||text(block).length<5)continue;
  const old=before.filter(b=>text(b)===text(block)),next=after.filter(b=>text(b)===text(block));
  if(old.length===1&&next.length===1)shifts.push(center(next[0])-center(block));
 }
 if(shifts.length<2)return false;
 const sorted=shifts.toSorted((a,b)=>a-b),shift=sorted[Math.floor(sorted.length/2)];
 if(shift>=-8||shifts.filter(v=>Math.abs(v-shift)<=5).length<2)return false;
 return current.some(b=>!prior.some(p=>Math.abs(center(p)+shift-center(b))<=8)&&after.every(other=>center(other)<=center(b)+8));
}
