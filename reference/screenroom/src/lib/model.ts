export interface Media {
  id:string; name:string; title:string; show?:string; year?:number; season?:number; episode?:number;
  source:string; path:string; size:number; poster?:string; backdrop?:string; overview?:string; tmdbId?:number; genres?:string[];
}
export interface Progress { seconds:number; duration:number; updated:number; watched:boolean }
export interface Profile { id:string; name:string; favorites:string[]; progress:Record<string,Progress> }
export interface Library { version:1; media:Media[]; profiles:Profile[]; activeProfile:string }
export function emptyLibrary():Library { return {version:1,media:[],profiles:[{id:'me',name:'Me',favorites:[],progress:{}}],activeProfile:'me'}; }
export const videoPattern=/\.(mp4|mkv|m4v|mov|webm|avi|ts|m2ts|mpg|mpeg)$/i;
function clean(s:string) {return s.replace(/[._]+/g,' ').replace(/\s+/g,' ').trim();}
export function parseName(name:string):Pick<Media,'title'|'show'|'season'|'episode'|'year'> {
  const stem=name.replace(/\.[^.]+$/,'');
  const ep=/\bS(\d{1,2})E(\d{1,3})\b/i.exec(stem.replace(/[._]/g,' ')) || /\b(\d{1,2})x(\d{1,3})\b/i.exec(stem.replace(/[._]/g,' '));
  if(ep){ const show=clean(stem.slice(0,ep.index)); return {title:show||'Unknown series',show:show||'Unknown series',season:Number(ep[1]),episode:Number(ep[2])}; }
  const yearMatch=/\b(19\d{2}|20\d{2})\b/.exec(stem.replace(/[._]/g,' '));
  const cut=yearMatch?yearMatch.index:stem.search(/\b(?:480p|720p|1080p|2160p|BluRay|WEB[- .]?DL|WEBRip|HDTV|x264|x265|HEVC)\b/i);
  return {title:clean(cut>=0?stem.slice(0,cut):stem).replace(/[\s([\]-]+$/,'')||clean(stem),year:yearMatch?Number(yearMatch[1]):undefined};
}
// A source path identifies a physical copy. Do not merge editions by title alone.
export function mediaId(source:string,path:string,size:number):string {
  return 'm-'+encodeURIComponent(JSON.stringify([source,path,size]));
}
export function makeMedia(name:string,path:string,size:number,source:string):Media {return {id:mediaId(source,path,size),name,path,size,source,...parseName(name)};}
export function mergeMedia(old:Media[],incoming:Media[]):Media[] {
  const items=new Map(old.map(m=>[m.id,m]));
  for(const item of incoming)items.set(item.id,{...items.get(item.id),...item});
  return [...items.values()];
}
export function nextEpisode(current:Media,media:Media[]):Media|undefined {
  if(!current.show||current.season===undefined||current.episode===undefined)return;
  // Never silently skip missing episodes or cross to another source/edition.
  return media.find(m=>m.source===current.source&&m.show===current.show&&m.season===current.season&&m.episode===current.episode!+1);
}
export function recordProgress(seconds:number,duration:number):Progress {
  const safeDuration=Number.isFinite(duration)?Math.max(0,duration):0;
  return {seconds:Math.min(Math.max(0,Number.isFinite(seconds)?seconds:0),safeDuration||Infinity),duration:safeDuration,updated:Date.now(),watched:safeDuration>0&&seconds/safeDuration>=0.95};
}
export function isLibrary(value:unknown):value is Library {
  if(!value||typeof value!=='object')return false;
  const x=value as Library;
  return x.version===1&&Array.isArray(x.media)&&x.media.length<=50000&&x.media.every(m=>m&&typeof m.id==='string'&&typeof m.title==='string'&&typeof m.name==='string'&&typeof m.path==='string'&&typeof m.source==='string'&&Number.isFinite(m.size)&&(!m.poster||/^https:\/\/image\.tmdb\.org\//.test(m.poster))&&(!m.backdrop||/^https:\/\/image\.tmdb\.org\//.test(m.backdrop)))&&Array.isArray(x.profiles)&&x.profiles.length>0&&x.profiles.every(p=>p&&typeof p.id==='string'&&typeof p.name==='string'&&Array.isArray(p.favorites)&&p.favorites.every(id=>typeof id==='string')&&p.progress&&typeof p.progress==='object'&&Object.values(p.progress).every(v=>v&&Number.isFinite(v.seconds)&&Number.isFinite(v.duration)&&Number.isFinite(v.updated)&&typeof v.watched==='boolean'))&&x.profiles.some(p=>p.id===x.activeProfile);
}
export function formatTime(s:number){return `${Math.floor(s/60)}:${String(Math.floor(s%60)).padStart(2,'0')}`;}
