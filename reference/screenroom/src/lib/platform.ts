import { makeMedia,videoPattern } from './model';
import type { Media } from './model';
declare global {interface Window {tizen?:any; webapis?:any; __TAURI_INTERNALS__?:unknown} }
export const isDesktop=()=>Boolean(window.__TAURI_INTERNALS__);
export const isTV=()=>Boolean(window.tizen&&window.webapis);
export const fileUrls=new Map<string,string>();
export function ingestFiles(files:File[]):Media[]{
  const result:Media[]=[];
  for(const f of files){if(!videoPattern.test(f.name))continue;
    const path=f.webkitRelativePath||f.name;
    const m=makeMedia(f.name,path,f.size,f.webkitRelativePath?path.split('/')[0]:'Picked files');
    const old=fileUrls.get(m.id);if(old)URL.revokeObjectURL(old);
    fileUrls.set(m.id,URL.createObjectURL(f));result.push(m);
  }return result;
}
export async function pickNativeFolder():Promise<Media[]>{
  const {invoke}=await import('@tauri-apps/api/core');
  const rows=await invoke<{name:string;path:string;size:number;source:string}[]>('pick_and_scan');
  const {convertFileSrc}=await import('@tauri-apps/api/core');
  return rows.map(r=>{const m=makeMedia(r.name,r.path,r.size,r.source);fileUrls.set(m.id,convertFileSrc(r.path));return m;});
}
export async function tvStorages():Promise<{label:string;state:string}[]>{
  return new Promise((resolve,reject)=>window.tizen.filesystem.listStorages(resolve,reject));
}
export async function scanTV(storage:string):Promise<Media[]>{
  const root:any=await new Promise((resolve,reject)=>window.tizen.filesystem.resolve(storage,resolve,reject,'r'));
  const results:Media[]=[]; const stack=[root];
  while(stack.length){const dir=stack.pop();const children:any[]=await new Promise((resolve,reject)=>dir.listFiles(resolve,reject));
    for(const file of children){if(file.isDirectory)stack.push(file);else if(videoPattern.test(file.name)){
      const uri=file.toURI();const m=makeMedia(file.name,uri,file.fileSize||0,storage);fileUrls.set(m.id,uri);results.push(m);
    }}
    await new Promise(resolve=>setTimeout(resolve,0));
  }return results;
}
export interface NativeTVPlayer {stop():void;toggle():void;seek(seconds:number):void}
export async function playTV(url:string,start:number,onTime:(seconds:number,duration:number)=>void,onEnd:()=>void,onError:(message:string)=>void):Promise<NativeTVPlayer>{
  const p=window.webapis.avplay;try{p.stop();p.close();}catch{}
  p.open(url);p.setDisplayRect(0,0,1920,1080);p.setDisplayMethod('PLAYER_DISPLAY_MODE_LETTER_BOX');
  p.setListener({oncurrentplaytime:(ms:number)=>onTime(ms/1000,p.getDuration()/1000),onstreamcompleted:onEnd,onerror:(e:unknown)=>onError('TV playback failed: '+String(e))});
  await new Promise<void>((resolve,reject)=>p.prepareAsync(()=>resolve(),reject));
  if(start>0&&start*1000<p.getDuration()-1000)await new Promise<void>((resolve,reject)=>p.seekTo(start*1000,resolve,reject));
  p.play();return {stop(){try{p.stop();p.close();}catch{}},toggle(){p.getState()==='PLAYING'?p.pause():p.play();},seek(s){p.seekTo(Math.max(1,Math.min(p.getDuration()-1000,s*1000)));}};
}
