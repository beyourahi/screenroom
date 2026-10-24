<script lang="ts">
  import {onMount,tick} from 'svelte';
  import {emptyLibrary,isLibrary,mergeMedia,nextEpisode,recordProgress,formatTime} from './lib/model';
  import type {Library,Media} from './lib/model';
  import {ingestFiles,pickNativeFolder,fileUrls,isDesktop,isTV,tvStorages,scanTV,playTV} from './lib/platform';
  import type {NativeTVPlayer} from './lib/platform';
  import {findMatches,applyMatch} from './lib/metadata';
  import type {Match} from './lib/metadata';
  let library=$state<Library>(emptyLibrary());
  let view=$state('Home'),search=$state(''),filter=$state('All'),sort=$state('Newest first');
  let message=$state(''),busy=$state(false),settings=$state(false),profileName=$state('');
  let selected=$state<Media|null>(null),active=$state<Media|null>(null),token=$state('');
  let matches=$state<Match[]>([]),metadataBusy=$state(false),seconds=$state(0),duration=$state(0);
  let storages=$state<{label:string;state:string}[]>([]),tvSource=$state('');
  let filesInput:HTMLInputElement,backupInput:HTMLInputElement,subtitleInput:HTMLInputElement;
  let playerDialog:HTMLDialogElement; let video=$state<HTMLVideoElement|undefined>(); let tvPlayer:NativeTVPlayer|undefined;
  let saveStamp=0,subtitleUrl='',resumeTarget=0,loading=false;let subtitleSource=$state('');
  const profile=$derived(library.profiles.find(p=>p.id===library.activeProfile)||library.profiles[0]);
  const visible=$derived(library.media.filter(m=>(view!=='Watchlist'||profile.favorites.includes(m.id))&&(filter==='All'||(filter==='Series'?Boolean(m.show):!m.show))&&`${m.title} ${m.name} ${m.year||''}`.toLowerCase().includes(search.toLowerCase())).sort((a,b)=>sort==='Title'?a.title.localeCompare(b.title,undefined,{numeric:true}):library.media.indexOf(b)-library.media.indexOf(a)));
  const continuing=$derived(library.media.filter(m=>profile.progress[m.id]?.seconds>0&&!profile.progress[m.id]?.watched).sort((a,b)=>profile.progress[b.id].updated-profile.progress[a.id].updated));
  const favoriteCount=$derived(profile.favorites.length);
  function notify(text:string){message=text;}
  function persist(){try{localStorage.setItem('screenroom-v1',JSON.stringify($state.snapshot(library)));}catch{notify('Could not save the library. Export a backup before closing.');}}
  function label(m:Media){return m.show?`S${String(m.season).padStart(2,'0')} · E${String(m.episode).padStart(2,'0')}`:m.year?String(m.year):'Movie';}
  function favorite(m:Media){profile.favorites=profile.favorites.includes(m.id)?profile.favorites.filter(id=>id!==m.id):[...profile.favorites,m.id];persist();}
  async function addMedia(){
    if(busy)return;
    if(isTV()){
      try{storages=await tvStorages();tvSource=storages.find(s=>s.state==='MOUNTED'||s.state==='mounted')?.label||storages[0]?.label||'';settings=true;}
      catch(e){notify('Cannot list TV storage: '+String(e));}return;
    }
    if(!isDesktop()){filesInput.click();return;}
    busy=true;
    try{const rows=await pickNativeFolder();library.media=mergeMedia(library.media,rows);persist();notify(`${rows.length} videos connected. Originals were not changed.`);}
    catch(e){notify(String(e));}finally{busy=false;}
  }
  function importFiles(event:Event){const input=event.currentTarget as HTMLInputElement;const rows=ingestFiles(Array.from(input.files||[]));library.media=mergeMedia(library.media,rows);persist();notify(`${rows.length} videos connected. Re-select the folder after restarting to reconnect its files.`);input.value='';}
  async function connectTV(){busy=true;try{const rows=await scanTV(tvSource);library.media=mergeMedia(library.media,rows);persist();notify(`${rows.length} videos connected from ${tvSource}.`);}catch(e){notify(String(e));}finally{busy=false;}}
  function checkpoint(force=false){if(!active||loading)return;if(!force&&Date.now()-saveStamp<2000)return;saveStamp=Date.now();profile.progress={...profile.progress,[active.id]:recordProgress(seconds,duration)};persist();}
  function timeUpdate(){if(video&&!loading){seconds=video.currentTime;duration=Number.isFinite(video.duration)?video.duration:0;checkpoint();}}
  async function start(m:Media,restart=false){
    const url=fileUrls.get(m.id);if(!url){notify('Reconnect the folder or USB drive, then play this item again. Your history is saved.');await addMedia();return;}
    checkpoint(true);tvPlayer?.stop();tvPlayer=undefined;video?.pause();
    loading=true;active=m;seconds=restart?0:profile.progress[m.id]?.watched?0:profile.progress[m.id]?.seconds||0;duration=profile.progress[m.id]?.duration||0;resumeTarget=seconds;
    subtitleSource='';if(subtitleUrl){URL.revokeObjectURL(subtitleUrl);subtitleUrl='';}
    await tick();if(!playerDialog.open)playerDialog.showModal();
    if(isTV()){
      document.documentElement.classList.add('tv-playing');
      try{tvPlayer=await playTV(url,seconds,(s,d)=>{seconds=s;duration=d;checkpoint();},ended,notify);loading=false;}catch(e){notify('Cannot start TV playback: '+String(e));closePlayer();}
    }else if(video){video.src=url;video.load();}
  }
  async function ready(){if(!video)return;duration=Number.isFinite(video.duration)?video.duration:0;if(resumeTarget>0&&resumeTarget<duration-1)video.currentTime=resumeTarget;loading=false;try{await video.play();}catch{notify('Press Play to begin playback.');}}
  function closePlayer(){checkpoint(true);video?.pause();tvPlayer?.stop();tvPlayer=undefined;active=null;loading=false;document.documentElement.classList.remove('tv-playing');if(playerDialog?.open)playerDialog.close();if(subtitleUrl){URL.revokeObjectURL(subtitleUrl);subtitleUrl='';}}
  async function ended(){if(!active)return;seconds=duration;checkpoint(true);const next=nextEpisode(active,library.media);if(next&&fileUrls.has(next.id))await start(next,true);else closePlayer();}
  function toggle(){if(tvPlayer)tvPlayer.toggle();else if(video)video.paused?void video.play().catch(e=>notify(String(e))):video.pause();}
  function seek(value:number){if(tvPlayer)tvPlayer.seek(value);else if(video)video.currentTime=value;seconds=value;}
  function playbackError(){notify('This device could not decode the file. A native broad-format desktop decoder and computer conversion are not integrated in this build.');}
  async function lookup(){if(!selected||!token.trim()){notify('Enter your personal TMDB Read Access Token to search metadata.');return;}metadataBusy=true;matches=[];try{matches=await findMatches(selected,token);if(!matches.length)notify('No match found. The video remains playable.');}catch(e){notify(String(e));}finally{metadataBusy=false;}}
  function chooseMatch(match:Match){if(!selected)return;const updated=applyMatch(selected,match);library.media=library.media.map(m=>m.id===updated.id?updated:m);selected=updated;matches=[];persist();notify('Match saved. Artwork currently needs internet access; durable offline artwork caching is still pending.');}
  function addProfile(){const name=profileName.trim();if(!name)return;const id='p-'+Date.now().toString(36);library.profiles=[...library.profiles,{id,name,favorites:[],progress:{}}];library.activeProfile=id;profileName='';persist();}
  function exportBackup(){const blob=new Blob([JSON.stringify($state.snapshot(library),null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='screenroom-library.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
  async function importBackup(event:Event){const input=event.currentTarget as HTMLInputElement;const file=input.files?.[0];if(!file)return;try{if(file.size>20_000_000)throw new Error('Backup exceeds 20 MB.');const parsed=JSON.parse(await file.text());if(!isLibrary(parsed))throw new Error('Unsupported or invalid backup.');for(const url of fileUrls.values())if(url.startsWith('blob:'))URL.revokeObjectURL(url);fileUrls.clear();library=parsed;selected=null;persist();notify('Library restored. Reconnect your media folders to play.');}catch(e){notify(String(e));}input.value='';}
  async function loadSubtitle(event:Event){const input=event.currentTarget as HTMLInputElement;const file=input.files?.[0];if(!file)return;if(file.size>5_000_000){notify('Subtitle file exceeds 5 MB.');return;}let text=await file.text();if(/\.srt$/i.test(file.name))text='WEBVTT\n\n'+text.replace(/\r/g,'').replace(/^(\d{2}:\d{2}:\d{2}),(\d{3}) --> (\d{2}:\d{2}:\d{2}),(\d{3})/gm,'$1.$2 --> $3.$4');if(subtitleUrl)URL.revokeObjectURL(subtitleUrl);subtitleUrl=URL.createObjectURL(new Blob([text],{type:'text/vtt'}));subtitleSource=subtitleUrl;await tick();if(video?.textTracks[0])video.textTracks[0].mode='showing';input.value='';}
  function navigate(event:KeyboardEvent){
    const target=event.target as HTMLElement;
    if(active){if(event.key==='Escape'||event.keyCode===10009){event.preventDefault();closePlayer();}else if((event.key===' '||event.keyCode===10252)&&!['INPUT','SELECT'].includes(target.tagName)){event.preventDefault();toggle();}return;}
    if(['INPUT','SELECT','TEXTAREA'].includes(target.tagName))return;
    if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;
    const candidates=Array.from(document.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')).filter(el=>el.getClientRects().length);
    const origin=target.getBoundingClientRect();const ox=origin.left+origin.width/2,oy=origin.top+origin.height/2;
    let best:HTMLButtonElement|undefined,bestScore=Infinity;
    for(const el of candidates){if(el===target)continue;const r=el.getBoundingClientRect(),dx=r.left+r.width/2-ox,dy=r.top+r.height/2-oy;const horizontal=event.key==='ArrowRight'||event.key==='ArrowLeft';const along=horizontal?dx:dy;const across=horizontal?dy:dx;if((event.key==='ArrowRight'||event.key==='ArrowDown')?along<=1:along>=-1)continue;const score=Math.abs(along)+Math.abs(across)*3;if(score<bestScore){bestScore=score;best=el;}}
    if(best){event.preventDefault();best.focus();best.scrollIntoView({block:'nearest',inline:'nearest'});}
  }
  onMount(()=>{
    try{const value=JSON.parse(localStorage.getItem('screenroom-v1')||'null');if(isLibrary(value))library=value;else if(value)notify('Saved library could not be read. Import a valid backup to restore it.');}catch{notify('Local storage is unavailable or invalid.');}
    if(isTV()){try{window.tizen.tvinputdevice.registerKey('MediaPlayPause');}catch{}}
    const hidden=()=>{if(document.hidden){checkpoint(true);video?.pause();if(tvPlayer)try{window.webapis.avplay.pause();}catch{}}};
    const exit=()=>checkpoint(true);document.addEventListener('visibilitychange',hidden);window.addEventListener('pagehide',exit);
    return()=>{document.removeEventListener('visibilitychange',hidden);window.removeEventListener('pagehide',exit);for(const url of fileUrls.values())if(url.startsWith('blob:'))URL.revokeObjectURL(url);};
  });
</script>

<svelte:window onkeydown={navigate}/>
<input class="hidden" bind:this={filesInput} type="file" webkitdirectory multiple onchange={importFiles} aria-label="Select a media folder"/>
<input class="hidden" bind:this={backupInput} type="file" accept="application/json,.json" onchange={importBackup} aria-label="Import library backup"/>
<input class="hidden" bind:this={subtitleInput} type="file" accept=".srt,.vtt" onchange={loadSubtitle} aria-label="Select subtitles"/>

<div class="shell">
  <header>
    <button class="wordmark" onclick={()=>view='Home'} aria-label="Screenroom home"><span class="brand-mark">s.</span>screenroom<span class="alpha">PERSONAL CINEMA</span></button>
    <nav aria-label="Main navigation">{#each ['Home','Library','Watchlist','Sources'] as tab}<button class:chosen={view===tab} onclick={()=>{view=tab;selected=null;}}>{tab}{#if tab==='Watchlist'&&favoriteCount}<span class="count">{favoriteCount}</span>{/if}</button>{/each}</nav>
    <div class="header-actions"><span class="local"><i></i> Local first</span><button class="profile" onclick={()=>settings=!settings} aria-label="Open settings and profiles">{profile.name.slice(0,1).toUpperCase()}</button></div>
  </header>

  {#if settings}
  <section class="settings" aria-label="Settings">
    <div class="section-heading"><h2>Your space</h2><button class="quiet" onclick={()=>settings=false}>Close</button></div>
    <div class="settings-grid"><label>Viewer<select bind:value={library.activeProfile} onchange={persist}>{#each library.profiles as p}<option value={p.id}>{p.name}</option>{/each}</select></label><label>New profile<input bind:value={profileName} placeholder="Name" maxlength="40"/></label><button onclick={addProfile}>Add profile</button></div>
    <div class="settings-grid"><label>TMDB Read Access Token · optional<input type="password" bind:value={token} autocomplete="off" placeholder="Used only for your searches; not saved"/></label></div>
    <p class="secondary">No account or cloud sync. History stays on this device. This early build uses your device’s decoder; libmpv, conversion, intro skipping, casting, and online subtitle search are still pending.</p>
    {#if storages.length}<div class="settings-grid"><label>TV storage<select bind:value={tvSource}>{#each storages as s}<option value={s.label}>{s.label} · {s.state}</option>{/each}</select></label><button disabled={busy||!tvSource} onclick={connectTV}>{busy?'Scanning…':'Scan selected storage'}</button></div>{/if}
    {#if !isTV()&&!isDesktop()}<div class="button-row"><button onclick={exportBackup}>Export library backup</button><button onclick={()=>backupInput.click()}>Restore backup</button></div>{/if}
    <p class="attribution">This product uses the TMDB API but is not endorsed or certified by TMDB. Metadata integration is experimental; required TMDB logo attribution must be added before distribution.</p>
  </section>{/if}

  {#if view==='Sources'}
    <main><section class="sources-hero"><span class="eyebrow">YOUR MEDIA, YOUR WAY</span><h1>Start at the source.</h1><p>Connect a collection. Keep the cinema here.</p></section><div class="source-grid"><article><span class="source-number">01</span><h2>Folders & external drives</h2><p>Read your media in place. Your original files stay untouched.</p><button class="primary" onclick={addMedia} disabled={busy}>Connect a folder <span>↗</span></button></article><article><span class="source-number">02</span><h2>Your familiar sources</h2><p>Open these websites yourself. Access depends on your network and each service’s requirements.</p><div class="external-links"><a href="https://cdn.dflix.live" target="_blank" rel="noopener noreferrer">Dflix <span>↗</span></a></div><small>Bookmarks only. No reachability, content rights, download, or streaming integration verified.</small></article></div></main>
  {:else}
    <main>
      {#if view==='Home'&&!search&&!library.media.length}
      <section class="hero"><div class="hero-copy"><span class="eyebrow">THE BEST SEAT IS YOURS</span><h1>Your collection.<br/><em>A better seat.</em></h1><p>Movies and series, beautifully together.<br/>No uploads. No subscriptions. Just press play.</p><div class="button-row"><button class="primary" onclick={addMedia} disabled={busy}>{busy?'Connecting…':library.media.length?'Connect more media':'Connect your media'} <span>+</span></button>{#if continuing[0]}<button class="outlined" onclick={()=>start(continuing[0])}>Continue watching <span>▶</span></button>{/if}</div><div class="hero-note">LOCAL FILES <span>·</span> EXTERNAL DRIVES <span>·</span> YOUR SCREEN</div></div><div class="hero-art" aria-hidden="true"><div class="orb"></div><div class="cinema-frame"><div class="frame-lines"></div><span class="frame-tag">A SPACE FOR EVERY STORY</span><div class="frame-type">stay<br/><em>for the</em><br/>credits.</div><span class="frame-bottom">SCREENROOM ORIGINAL SPACE / 001</span></div></div></section>
      {/if}
      {#if view==='Home'&&continuing.length&&!search}<section class="continue-section"><div class="section-heading"><h2>Pick up where you left off</h2><span class="secondary">For {profile.name}</span></div><div class="continue-grid">{#each continuing.slice(0,4) as m}<button class="continue-card" onclick={()=>start(m)}><span class="continue-art">▶</span><span class="continue-copy"><strong>{m.title}</strong><small>{label(m)} · {formatTime(profile.progress[m.id].seconds)} played</small><span class="progress-track"><span style:width={`${Math.min(100,profile.progress[m.id].seconds/(profile.progress[m.id].duration||1)*100)}%`}></span></span></span></button>{/each}</div></section>{/if}

      <section class="library-section"><div class="section-heading"><div><span class="eyebrow">CURATED BY YOU</span><h2>{view==='Watchlist'?'Saved for a good evening':'Your library'} <span class="library-count">{visible.length}</span></h2></div><button class="quiet" onclick={addMedia} disabled={busy}>+ Add media</button></div>
        <div class="toolbar"><div class="filters" aria-label="Media type">{#each ['All','Movies','Series'] as f}<button aria-pressed={filter===f} onclick={()=>filter=f}>{f}</button>{/each}</div><div class="search-sort"><input aria-label="Search library" bind:value={search} placeholder="Search library…"/><select aria-label="Sort library" bind:value={sort}><option>Newest first</option><option>Title</option></select></div></div>
        {#if !visible.length}<div class="empty"><span class="empty-symbol">+</span><h3>{library.media.length?'Nothing here just yet.':'Your next great watch starts here.'}</h3><p>{library.media.length?'Try another filter or save a title to your watchlist.':'Choose a folder or USB drive. We’ll organize the files without moving them.'}</p>{#if !library.media.length}<button class="outlined" onclick={addMedia}>Choose a media folder</button>{/if}</div>
        {:else}<div class="poster-grid">{#each visible.slice(0,120) as m}<article class="media-card"><button class="poster" onclick={()=>{selected=m;matches=[];}} aria-label={`Details for ${m.title}, ${label(m)}`}>{#if m.poster}<img src={m.poster} alt="" loading="lazy"/>{:else}<div class="poster-fallback"><span>{m.show?'SERIES':'FILM'}</span><strong>{m.title}</strong><small>{m.year||'YOUR COLLECTION'}</small></div>{/if}<span class="poster-play">▶</span>{#if profile.progress[m.id]?.watched}<span class="watched">WATCHED</span>{/if}</button><div class="card-meta"><div><h3>{m.title}</h3><small>{label(m)}</small></div><button class="save" aria-pressed={profile.favorites.includes(m.id)} aria-label={`${profile.favorites.includes(m.id)?'Remove':'Add'} ${m.title} ${profile.favorites.includes(m.id)?'from':'to'} watchlist`} onclick={()=>favorite(m)}>{profile.favorites.includes(m.id)?'✓':'+'}</button></div></article>{/each}</div>{#if visible.length>120}<p class="secondary">Showing the first 120 results. Search to narrow your collection.</p>{/if}{/if}
      </section>
      {#if selected}<section class="details" aria-label="Selected title"><div class="section-heading"><span class="eyebrow">NOW SELECTED</span><button class="quiet" onclick={()=>selected=null}>Close details</button></div><h2>{selected.title}</h2><span class="secondary">{label(selected)} · {selected.source}</span><p>{selected.overview||'No description yet. Watch now, or match this file to a title when you’re online.'}</p><p class="filename">{selected.name}</p><div class="button-row"><button class="primary" onclick={()=>selected&&start(selected)}>▶ {profile.progress[selected.id]?.seconds&&!profile.progress[selected.id]?.watched?'Resume':'Play'}</button><button onclick={()=>selected&&favorite(selected)}>{profile.favorites.includes(selected.id)?'✓ In watchlist':'+ Watchlist'}</button><button disabled={metadataBusy} onclick={lookup}>{metadataBusy?'Searching…':'Find metadata'}</button></div>{#if !token}<p class="secondary">Optional metadata search: enter your personal token in profile settings.</p>{/if}{#if matches.length}<div class="match-list" aria-label="Metadata matches">{#each matches.slice(0,6) as match}<button onclick={()=>chooseMatch(match)}>{match.title||match.name} <span>{match.release_date?.slice(0,4)||match.first_air_date?.slice(0,4)}</span></button>{/each}</div>{/if}</section>{/if}
    </main>
  {/if}
  <footer><span>screenroom <span class="secondary">/ a personal cinema</span></span><span class="secondary">Your files. Your history. Your space.</span></footer>
</div>

{#if message}<aside class="toast" role="status"><span>{message}</span><button aria-label="Dismiss message" onclick={()=>message=''}>×</button></aside>{/if}

<dialog bind:this={playerDialog} class="player-dialog" oncancel={(e)=>{e.preventDefault();closePlayer();}}>
  {#if active}<div class="player-heading"><div><strong>{active.title}</strong><small>{label(active)}</small></div><button onclick={closePlayer}>Close player ×</button></div>
    {#if !isTV()}<video bind:this={video} controls playsinline preload="metadata" onloadedmetadata={ready} ontimeupdate={timeUpdate} onpause={()=>checkpoint(true)} onended={ended} onerror={playbackError}>{#if subtitleSource}<track kind="subtitles" src={subtitleSource} srclang="en" label="Imported subtitles" default/>{/if}</video><div class="player-footer"><button onclick={()=>subtitleInput.click()}>Add .srt / .vtt subtitles</button><span class="secondary">Space: pause · Esc: close · Original aspect ratio</span></div>
    {:else}<div class="tv-video-space"></div><div class="player-footer"><button onclick={toggle}>Play / pause</button><button onclick={()=>seek(Math.max(1,seconds-10))}>−10s</button><button onclick={()=>seek(Math.min(duration-1,seconds+10))}>+10s</button><span>{formatTime(seconds)} / {formatTime(duration)}</span></div>{/if}
  {/if}
</dialog>
