/** Retains independent Live TV decoders while switching the main tile, layout, and single audio source. */
import * as api from '../liveTvApi';
import type { TvPlayback } from './types';
import type { PlaybackProblem, PlaybackQuality } from './playbackOptions';
import { state } from './state';
import { escapeHtml as escape } from './format';
import { renderIcon } from './ui';
import { tvChannelPlayback } from './tvPlayer';
import { TvPlaybackWatchdog } from './tvPlaybackRecovery';
import { bindTvPlaybackHeadroom, tvLivePosition } from './tvPlaybackHeadroom';
import { channelPrograms } from './liveTvGuideModel';
import { openTvMultiviewPicker, closeTvMultiviewPicker } from './tvMultiviewPicker';
import { createIcons, icons } from 'lucide';
import { setElementHtml } from './domPatcher';

/** Four viewing buffers can run concurrently, subject to each source's tuner allowance. */
export const TV_MULTIVIEW_LIMIT = 4;
/** Per-tile state survives promotion to and from the shared main player. */
export interface TvMultiviewTile { playback:TvPlayback; quality:PlaybackQuality; paused:boolean; problem?:PlaybackProblem; pending?:boolean }
/** A viewing arrangement belongs to the current account and Live TV screen only. */
export interface TvMultiview { owner:number; layout:'grid'|'focus'; tiles:TvMultiviewTile[] }
interface Callbacks {render:()=>void;beforeMainChange:()=>void;stop:()=>void}
let callbacks:Callbacks;
let releasing:Promise<unknown>=Promise.resolve();
let boundRoot:Element|null=null,events:AbortController|undefined;
const satellites=new Map<TvMultiviewTile,{video:HTMLVideoElement;dispose:()=>void}>();
const pendingTunes=new Map<TvMultiviewTile,Promise<void>>();
let attempt=Date.now();
let sizing:ResizeObserver|undefined,sizedControls:Element|null=null;

/** Use the existing player coordinator for all main-stream controls and route renders. */
export function configureTvMultiview(input:Callbacks):void { callbacks=input; }

function current(view:TvMultiview,tile:TvMultiviewTile):boolean {
  return state.tvMultiview===view&&view.owner===state.bootstrap?.current_user?.id&&state.route.page==='live-tv'&&view.tiles.includes(tile);
}
function release(tile:TvMultiviewTile):void {
  const buffer=tile.playback.buffer,pending=pendingTunes.get(tile);
  if(buffer)releasing=Promise.allSettled([releasing,api.closeTvBuffer(buffer.session_id)]);
  if(pending)releasing=Promise.allSettled([releasing,pending]);
}
function videoFor(id:number):HTMLVideoElement|null {return document.querySelector<HTMLVideoElement>(`#tv-stream-${id} video`);}

/** Release every extra tuner and pending tune; the standard player releases its main buffer. */
export function stopTvMultiview():Promise<unknown> {
  const view=state.tvMultiview;state.tvMultiview=undefined;
  closeTvMultiviewPicker();events?.abort();boundRoot=null;
  sizing?.disconnect();sizedControls=null;
  for(const runtime of satellites.values()){runtime.dispose();}satellites.clear();
  for(const tile of view?.tiles??[]){
    if(tile.playback===state.activeTvPlayback)continue;
    const video=videoFor(tile.playback.id);video?.pause();video?.removeAttribute('src');video?.load();release(tile);
  }
  return releasing;
}

/** Browser exit also releases background tiles using authenticated keepalive requests. */
export function finishTvMultiviewOnExit():void {
  for(const tile of state.tvMultiview?.tiles??[])if(tile.playback!==state.activeTvPlayback&&tile.playback.buffer)api.closeTvBufferOnExit(tile.playback.buffer.session_id);
}

/** Keep the ordinary single video in a stable tile, ready for additional streams without moving its decoder. */
export function renderTvStage(media:string):string {
  const tv=state.activeTvPlayback;
  if(tv?.kind!=='channels')return media;
  const tiles=state.tvMultiview?.tiles??[{playback:tv}];
  const markup=tiles.map(tile=>{
    const main=tile.playback===tv,stream=tile.playback;
    let video=media;
    if(!main)video=stream.url?`<video id="tv-extra-video-${stream.id}" src="${escape(stream.url)}" autoplay muted playsinline preload="auto" data-playback-offset-ms="${stream.startMs??0}"></video>`:'';
    return `<section id="tv-stream-${stream.id}" class="tv-stream-tile${main?' is-main':''}" aria-label="${escape(stream.title)} stream">${video}<div class="tv-stream-heading"></div><div class="tv-stream-problem" hidden></div></section>`;
  }).join('');
  let className='';if(state.tvMultiview)className=`tv-multiview-stage tv-layout-${state.tvMultiview.layout}`;
  return `<div id="tv-player-stage" class="${className}">${markup}</div>`;
}

/** Add streams from the existing player's toolbar, rather than introducing a second player. */
export function renderTvMultiviewButton():string {
  if(state.activeTvPlayback?.kind!=='channels')return '';
  const label=state.tvMultiview?'Add stream':'Multiview';
  return `<button id="player-multiview" type="button" class="player-icon-button" aria-label="${label}" title="${label}">${renderIcon('layout-grid','player-control-icon')}</button>`;
}

function ensureView():TvMultiview|undefined {
  const tv=state.activeTvPlayback;if(tv?.kind!=='channels'||tv.owner!==state.bootstrap?.current_user?.id)return;
  if(!state.tvMultiview){
    tv.guideOpen=false;
    state.tvMultiview={owner:tv.owner,layout:'focus',tiles:[{playback:tv,quality:state.activePlaybackQuality??'original',paused:false,pending:!tv.url&&!state.playbackProblem}]};
    callbacks.render();
  }
  return state.tvMultiview;
}

/** Reserve a tile immediately so repeated clicks and concurrent tunes cannot exceed four streams. */
export async function addTvMultiviewChannel(channel:api.TvChannel):Promise<void> {
  const view=ensureView();if(!view||view.tiles.length>=TV_MULTIVIEW_LIMIT||view.tiles.some(t=>t.playback.id===channel.id))return;
  const tile:TvMultiviewTile={playback:{...tvChannelPlayback(channel),url:'',owner:view.owner},quality:'original',paused:false,pending:true};
  view.tiles.push(tile);callbacks.render();
  await trackTune(view,tile,channel);
}

async function trackTune(view:TvMultiview,tile:TvMultiviewTile,channel:api.TvChannel):Promise<void> {
  const operation=tuneTile(view,tile,channel);pendingTunes.set(tile,operation);
  try{await operation;}finally{pendingTunes.delete(tile);}
}

async function tuneTile(view:TvMultiview,tile:TvMultiviewTile,channel:api.TvChannel):Promise<void> {
  try{
    await releasing;if(!current(view,tile)){return;}
    const buffer=await api.startTvBuffer(channel.id);
    if(!current(view,tile)){await api.closeTvBuffer(buffer.session_id);return;}
    tile.playback.buffer=buffer;tile.playback.startMs=tvLivePosition(tile.playback);
    tile.playback.url=api.tvBufferStreamUrl(buffer.session_id,tile.playback.startMs,tile.quality,++attempt);
    if(state.liveTv?.owner===view.owner){const preferences=state.liveTv.preferences??{favorites:[],recent:[]};state.liveTv.preferences={...preferences,recent:[channel.id,...preferences.recent.filter(id=>id!==channel.id)].slice(0,30)};}
  }catch(error){if(current(view,tile))tile.problem={message:error instanceof Error?error.message:'Unable to start this stream.',retry:true};}
  finally{if(current(view,tile)){tile.pending=false;if(tile.playback===state.activeTvPlayback){state.playbackProblem=tile.problem;}callbacks.render();}}
}

/** Promotion changes controls, audio, and CSS placement; it never retunes either channel. */
export function promoteTvMultiview(id:number):void {
  const view=state.tvMultiview,tile=view?.tiles.find(t=>t.playback.id===id);
  if(!view||!tile||tile.playback===state.activeTvPlayback)return;
  const previous=view.tiles.find(t=>t.playback===state.activeTvPlayback),oldVideo=document.querySelector<HTMLVideoElement>('#media-player');
  const volume=oldVideo?.volume??state.playbackSettings?.volume??1,muted=oldVideo?.muted??state.playbackSettings?.muted??false;
  if(previous){previous.paused=oldVideo?.paused??false;previous.quality=state.activePlaybackQuality??'original';previous.problem=state.playbackProblem;}
  callbacks.beforeMainChange();
  if(oldVideo){oldVideo.id=`tv-extra-video-${previous?.playback.id}`;oldVideo.muted=true;}
  satellites.get(tile)?.dispose();satellites.delete(tile);
  const video=videoFor(id);if(video){video.id='media-player';video.volume=volume;video.muted=muted;}
  state.activeTvPlayback=tile.playback;state.activePlaybackQuality=tile.quality;state.playbackProblem=tile.problem;
  state.playbackSettings={paused:tile.paused,volume,muted};callbacks.render();
}

function removeTile(id:number):void {
  const view=state.tvMultiview,tile=view?.tiles.find(t=>t.playback.id===id);if(!view||!tile)return;
  if(view.tiles.length===1){callbacks.stop();return;}
  if(tile.playback===state.activeTvPlayback)promoteTvMultiview(view.tiles.find(t=>t!==tile)!.playback.id);
  view.tiles=view.tiles.filter(t=>t!==tile);satellites.get(tile)?.dispose();satellites.delete(tile);
  const video=videoFor(id);video?.pause();video?.removeAttribute('src');video?.load();release(tile);callbacks.render();
}

async function retryTile(tile:TvMultiviewTile):Promise<void> {
  const view=state.tvMultiview;if(!view||tile.pending)return;
  const channel=state.liveTv?.channels.find(c=>c.id===tile.playback.id);if(!channel)return;
  satellites.get(tile)?.dispose();satellites.delete(tile);
  release(tile);tile.playback.buffer=undefined;tile.playback.url='';tile.problem=undefined;tile.pending=true;tile.paused=false;
  if(tile.playback===state.activeTvPlayback){state.playbackProblem=undefined;callbacks.beforeMainChange();}
  callbacks.render();await trackTune(view,tile,channel);
}

/** Retune only a failed main tile, leaving the other viewing sessions running. */
export async function retryTvMultiviewMain():Promise<void> {
  const tile=state.tvMultiview?.tiles.find(t=>t.playback===state.activeTvPlayback);if(tile)await retryTile(tile);
}

function watchMainOnly():void {
  const video=document.querySelector<HTMLVideoElement>('#media-player');
  if(video)state.playbackSettings={paused:video.paused,volume:video.volume,muted:video.muted};
  callbacks.beforeMainChange();void stopTvMultiview();callbacks.render();
  if(!state.playbackSettings?.paused)void document.querySelector<HTMLVideoElement>('#media-player')?.play().catch(()=>undefined);
}

function tileTitle(tile:TvMultiviewTile):string {
  const data=state.liveTv,channel=data?.channels.find(c=>c.id===tile.playback.id),now=Date.now()/1000;
  return channel&&data?channelPrograms(channel,data.programs).find(p=>p.starts_at<=now&&p.ends_at>now)?.title??'': '';
}
function heading(tile:TvMultiviewTile,main:boolean):string {
  const tv=tile.playback,logo=tv.logo?`<img data-tv-image data-fallback-src="${escape(tv.logoFallback??'')}" src="${escape(tv.logo)}" alt="" referrerpolicy="no-referrer"/>`:'';
  return `<button type="button" data-tv-multi-main="${tv.id}" aria-label="${main?'Main stream':'Make main and listen to'} ${escape(tv.title)}" aria-pressed="${main}" title="Make main and listen">${logo}<span><strong>${escape(tv.title)}</strong><small>${escape(tileTitle(tile))}</small></span>${renderIcon(main?'volume-2':'volume-x')}</button><button type="button" data-tv-multi-remove="${tv.id}" aria-label="Remove ${escape(tv.title)}" title="Remove stream">${renderIcon('x')}</button>`;
}

function syncTile(tile:TvMultiviewTile,stage:HTMLElement):void {
  const main=tile.playback===state.activeTvPlayback;
  let element=stage.querySelector<HTMLElement>(`#tv-stream-${tile.playback.id}`);
  if(!element){element=document.createElement('section');element.id=`tv-stream-${tile.playback.id}`;element.className='tv-stream-tile';element.setAttribute('aria-label',`${tile.playback.title} stream`);element.innerHTML='<div class="tv-stream-heading"></div><div class="tv-stream-problem" hidden></div>';stage.append(element);}
  element.classList.toggle('is-main',main);
  const title=element.querySelector<HTMLElement>('.tv-stream-heading')!,markup=heading(tile,main);
  if(title.dataset.content!==markup){setElementHtml(title,markup);title.dataset.content=markup;}
  const notice=element.querySelector<HTMLElement>('.tv-stream-problem')!;
  notice.setAttribute('role','status');
  notice.hidden=main||(!tile.pending&&!tile.problem);
  const error=tile.problem?`${escape(tile.problem.message)}<button type="button" data-tv-multi-retry="${tile.playback.id}">Retry stream</button>`:'Starting stream…';
  if(notice.dataset.content!==error){notice.innerHTML=error;notice.dataset.content=error;}
  if(main){satellites.get(tile)?.dispose();satellites.delete(tile);return;}
  let video=element.querySelector<HTMLVideoElement>('video');
  if(!video){video=document.createElement('video');video.id=`tv-extra-video-${tile.playback.id}`;video.playsInline=true;video.preload='auto';video.muted=true;element.prepend(video);}
  video.muted=true;
  if(video.getAttribute('src')!==tile.playback.url&&tile.playback.url){video.src=tile.playback.url;video.dataset.playbackOffsetMs=String(tile.playback.startMs??0);if(!tile.paused)void video.play().catch(()=>undefined);}
  if(satellites.get(tile)?.video!==video){satellites.get(tile)?.dispose();satellites.set(tile,{video,dispose:bindSatellite(tile,video)});}
}

function bindSatellite(tile:TvMultiviewTile,video:HTMLVideoElement):()=>void {
  const view=state.tvMultiview!,events=new AbortController(),watchdog=new TvPlaybackWatchdog();let polling=false;
  const position=()=>Number(video.dataset.playbackOffsetMs??tile.playback.startMs??0)+video.currentTime*1000;
  const active=()=>current(view,tile)&&tile.playback!==state.activeTvPlayback;
  const cleanupHeadroom=bindTvPlaybackHeadroom(video,tile.playback,active,()=>tile.paused);
  watchdog.attach(position(),Date.now());
  const recover=(interrupted=false)=>{
    const buffer=tile.playback.buffer;if(!active()||!buffer||buffer.error||buffer.finished||tile.paused)return;
    const result=watchdog.observe({position:position(),interrupted:interrupted||video.ended||Boolean(video.error),paused:video.paused&&!video.ended&&!video.error,unavailable:buffer.end_ms-buffer.start_ms<4000},Date.now());
    if(result==='recover'){
      const live=tvLivePosition(tile.playback);tile.playback.startMs=watchdog.attempts>1?live:Math.max(buffer.start_ms,Math.min(position(),live));
      tile.playback.url=api.tvBufferStreamUrl(buffer.session_id,tile.playback.startMs,tile.quality,++attempt);callbacks.render();
    }else if(result==='exhausted'&&!tile.problem){tile.problem={message:'This stream stopped. Retry or choose another channel.',retry:true};callbacks.render();}
  };
  video.addEventListener('ended',()=>recover(true),{signal:events.signal});video.addEventListener('error',()=>recover(true),{signal:events.signal});
  const timer=globalThis.setInterval(()=>{
    const buffer=tile.playback.buffer;if(!active()||!buffer||polling){return;}polling=true;
    void api.getTvBuffer(buffer.session_id).then(info=>{
      if(!active()){return;}tile.playback.buffer=info;
      if(info.error){tile.problem={message:info.error,retry:true};callbacks.render();}
      else if(position()<info.start_ms&&!tile.paused){tile.playback.startMs=info.start_ms;tile.playback.url=api.tvBufferStreamUrl(info.session_id,info.start_ms,tile.quality,++attempt);callbacks.render();}
      else recover();
    }).catch(()=>undefined).finally(()=>{polling=false;});
  },2000);
  return ()=>{events.abort();cleanupHeadroom();globalThis.clearInterval(timer);};
}

function promoteClickedTile(target:Element):void {
  const tile=target.closest<HTMLElement>('.tv-stream-tile');if(tile&&state.tvMultiview)promoteTvMultiview(Number(tile.id.slice('tv-stream-'.length)));
}

function handleAction(event:MouseEvent):void {
  const target=event.target as Element,button=target.closest<HTMLButtonElement>('button');
  if(!button){promoteClickedTile(target);return;}
  if(button.id==='player-multiview'||button.dataset.tvMultiAdd !== undefined){
    const view=ensureView();if(view&&view.tiles.length<TV_MULTIVIEW_LIMIT){openTvMultiviewPicker(view,addTvMultiviewChannel);}return;
  }
  if(button.dataset.tvMultiMain){promoteTvMultiview(Number(button.dataset.tvMultiMain));return;}
  if(button.dataset.tvMultiRemove){removeTile(Number(button.dataset.tvMultiRemove));return;}
  if(button.dataset.tvMultiRetry){const tile=state.tvMultiview?.tiles.find(t=>t.playback.id===Number(button.dataset.tvMultiRetry));if(tile){void retryTile(tile);}return;}
  if(button.dataset.tvMultiOnly !== undefined){watchMainOnly();return;}
  const view=state.tvMultiview;
  if(view&&['grid','focus'].includes(button.dataset.tvMultiLayout??'')){view.layout=button.dataset.tvMultiLayout as 'grid'|'focus';callbacks.render();}
}

/** Synchronize tiles in place; guide refreshes and layout changes never replace playing videos. */
export function syncTvMultiview():void {
  const root=document.querySelector('#media-player-overlay');
  if(root!==boundRoot){events?.abort();boundRoot=root;events=new AbortController();root?.addEventListener('click',event=>handleAction(event as MouseEvent),{signal:events.signal});}
  const view=state.tvMultiview,stage=document.querySelector<HTMLElement>('#tv-player-stage');
  if(!view||!stage)return;
  stage.className=`tv-multiview-stage tv-layout-${view.layout}`;stage.dataset.count=String(view.tiles.length);
  const ids=new Set(view.tiles.map(t=>`tv-stream-${t.playback.id}`));
  stage.querySelectorAll('.tv-stream-tile').forEach(tile=>{if(!ids.has(tile.id))tile.remove();});
  for(const tile of view.tiles)syncTile(tile,stage);
  for(const [tile,runtime] of satellites)if(!view.tiles.includes(tile)){runtime.dispose();satellites.delete(tile);}
  syncAddTile(view,stage);
  syncToolbar(root,view);
  if(root)createIcons({icons,root});
  observeSize(root);
}

function syncAddTile(view:TvMultiview,stage:HTMLElement):void {
  let empty=stage.querySelector<HTMLElement>('#tv-multiview-empty');
  if(view.tiles.length<TV_MULTIVIEW_LIMIT){if(!empty){empty=document.createElement('button');empty.id='tv-multiview-empty';empty.setAttribute('type','button');empty.dataset.tvMultiAdd='';empty.className='tv-stream-add';empty.innerHTML=`${renderIcon('plus')}<span>Add stream</span>`;stage.append(empty);}}
  else empty?.remove();
  if(empty&&view.tiles.length<TV_MULTIVIEW_LIMIT&&stage.lastElementChild!==empty)stage.append(empty);
}

function syncToolbar(root:Element|null,view:TvMultiview):void {
  if(!state.isPlayerOpen)return;
  let toolbar=root?.querySelector<HTMLElement>('#tv-multiview-toolbar');
  if(!toolbar){toolbar=document.createElement('div');toolbar.id='tv-multiview-toolbar';toolbar.className='tv-multiview-toolbar';root?.querySelector('.player-top-controls')?.prepend(toolbar);}
  const controls=`<strong>Multiview <span>${view.tiles.length} / ${TV_MULTIVIEW_LIMIT}</span></strong><div role="group" aria-label="Multiview layout"><button type="button" data-tv-multi-layout="focus" aria-label="Main and sidebar" aria-pressed="${view.layout==='focus'}" title="Main and sidebar">${renderIcon('layout-panel-left')}</button><button type="button" data-tv-multi-layout="grid" aria-label="Equal grid" aria-pressed="${view.layout==='grid'}" title="Equal grid">${renderIcon('layout-grid')}</button></div><button type="button" data-tv-multi-only>Watch main only</button>`;
  if(toolbar.dataset.content!==controls){setElementHtml(toolbar,controls);toolbar.dataset.content=controls;}
  const add=root?.querySelector<HTMLButtonElement>('#player-multiview');if(add)add.disabled=view.tiles.length>=TV_MULTIVIEW_LIMIT;
}

function observeSize(root:Element|null):void {
  const bottom=root?.querySelector('.player-bottom-controls'),shell=root?.querySelector<HTMLElement>('#media-player-shell');
  if(bottom&&bottom!==sizedControls&&shell){
    sizing?.disconnect();sizedControls=bottom;
    const measure=()=>{shell.style.setProperty('--tv-multiview-controls-height',`${bottom.getBoundingClientRect().height+8}px`);shell.style.setProperty('--tv-multiview-top-height',`${root?.querySelector('.player-top-controls')?.getBoundingClientRect().height??68}px`);};
    sizing=new ResizeObserver(measure);sizing.observe(bottom);const top=root?.querySelector('.player-top-controls');if(top){sizing.observe(top);}measure();
  }
}
