import type {Media} from './model';
export interface Match {id:number;title?:string;name?:string;release_date?:string;first_air_date?:string;poster_path?:string;backdrop_path?:string;overview?:string}
export async function findMatches(media:Media,token:string):Promise<Match[]>{
  const q=new URLSearchParams({query:media.show||media.title,language:'en-US',include_adult:'false'});
  if(!media.show&&media.year)q.set('primary_release_year',String(media.year));
  const controller=new AbortController();const timeout=setTimeout(()=>controller.abort(),12000);
  try{
    const response=await fetch(`https://api.themoviedb.org/3/search/${media.show?'tv':'movie'}?${q}`,{headers:{Authorization:`Bearer ${token.trim()}`},signal:controller.signal});
    if(!response.ok)throw new Error(`Metadata lookup failed (${response.status}).`);
    const data=await response.json();return data.results||[];
  }finally{clearTimeout(timeout);}
}
export function applyMatch(media:Media,match:Match):Media{
  const title=match.title||match.name||media.title;
  return {...media,title,show:media.show?title:undefined,tmdbId:match.id,overview:match.overview,poster:match.poster_path?'https://image.tmdb.org/t/p/w342'+match.poster_path:undefined,backdrop:match.backdrop_path?'https://image.tmdb.org/t/p/w1280'+match.backdrop_path:undefined};
}
