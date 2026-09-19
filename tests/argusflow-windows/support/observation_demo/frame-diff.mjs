/** 对已落盘的实际前后帧逐像素比较，不用相邻采样的变化量代替操作变化。 */
import { readFileSync } from 'node:fs';
import { resolve, sep } from 'node:path';
export function difference(directory,before,after){
 const root=resolve(directory);
 function load(name){
  const path=resolve(root,name);
  if(!path.startsWith(root+sep))throw Error('帧路径越界');
  const data=readFileSync(path);
  if(data.toString('ascii',0,2)!=='BM'||data.readUInt16LE(28)!==24||data.readUInt32LE(30)!==0)throw Error('仅支持采集器的24位BMP');
  const width=data.readInt32LE(18),height=-data.readInt32LE(22),offset=data.readUInt32LE(10),stride=(width*3+3)&~3;
  if(width<=0||height<=0||data.length!==offset+stride*height)throw Error('帧大小不完整');
  return {data,width,height,offset,stride};
 }
 const a=load(before),b=load(after);
 if(a.width!==b.width||a.height!==b.height)return {comparable:false,reason:'窗口尺寸变化'};
 let changed=0,left=a.width,top=a.height,right=0,bottom=0;
 for(let y=0;y<a.height;y++)for(let x=0;x<a.width;x++){
  const p=a.offset+y*a.stride+x*3,q=b.offset+y*b.stride+x*3;
  if(a.data[p]!==b.data[q]||a.data[p+1]!==b.data[q+1]||a.data[p+2]!==b.data[q+2]){changed++;left=Math.min(left,x);top=Math.min(top,y);right=Math.max(right,x+1);bottom=Math.max(bottom,y+1);}
 }
 return {comparable:true,changed_pixels:changed,region:changed?[left,top,right-left,bottom-top]:null};
}
