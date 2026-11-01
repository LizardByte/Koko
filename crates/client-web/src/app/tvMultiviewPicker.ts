/** Provides a bounded, searchable channel picker for adding independent multiview streams. */
import type { TvChannel } from '../liveTvApi';
import type { TvMultiview } from './tvMultiview';
import { state } from './state';
import { escapeHtml as escape } from './format';
import { channelArtwork, channelArtworkFallback, channelPrograms } from './liveTvGuideModel';
import { renderIcon } from './ui';
import { createIcons, icons } from 'lucide';

let dismiss:(()=>void)|undefined;
/** Dismiss the channel picker when playback or the owning account leaves Live TV. */
export function closeTvMultiviewPicker():void {dismiss?.();}

/** Render at most fifty results, even when a source exposes thousands of channels. */
export function openTvMultiviewPicker(view:TvMultiview,add:(channel:TvChannel)=>Promise<void>):void {
  closeTvMultiviewPicker();
  const data=state.liveTv;if(data?.owner!==view.owner)return;
  const previous=document.activeElement as HTMLElement|null,dialog=document.createElement('dialog');
  dialog.className='tv-program-dialog tv-multiview-picker';dialog.id='tv-multiview-picker';dialog.setAttribute('aria-labelledby','tv-multi-picker-title');
  const sources=data.guideSources?.map(s=>`<option value="${s.id}">${escape(s.name)}</option>`).join('')??'';
  dialog.innerHTML=`<header><div><p class="eyebrow">Multiview</p><h2 id="tv-multi-picker-title">Add a stream</h2></div><button type="button" class="secondary-button" data-multi-picker-close aria-label="Close add stream">${renderIcon('x')}</button></header><p>Choose another channel. Select a tile to make it the main stream and hear its audio.</p><label>Find a channel<input name="search" type="search" placeholder="Channel or program"/></label><div class="tv-multi-picker-filters"><label>Source<select name="source"><option value="">All sources</option>${sources}</select></label><label>Provider<select name="provider"><option value="">All providers</option></select></label><label class="tv-dialog-checkbox"><input name="favorites" type="checkbox"/> Favorites</label></div><p role="status" class="tv-multi-picker-count"></p><div class="tv-multi-picker-results"></div><footer><button type="button" class="secondary-button" data-multi-picker-page="-1">Previous</button><button type="button" class="secondary-button" data-multi-picker-page="1">Next</button><button type="button" class="secondary-button" data-multi-picker-close>Cancel</button></footer>`;
  const events=new AbortController(),signal=events.signal;
  const close=()=>{events.abort();dialog.close();dialog.remove();if(dismiss===close){dismiss=undefined;}if(state.tvMultiview===view&&previous?.isConnected)previous.focus({preventScroll:true});};
  dismiss=close;
  const input=dialog.querySelector<HTMLInputElement>('[name="search"]')!,source=dialog.querySelector<HTMLSelectElement>('[name="source"]')!,provider=dialog.querySelector<HTMLSelectElement>('[name="provider"]')!,favorites=dialog.querySelector<HTMLInputElement>('[name="favorites"]')!;
  source.setAttribute('aria-label','Source');provider.setAttribute('aria-label','Provider');
  const now=Date.now()/1000;
  const channels=data.channels.filter(c=>c.enabled&&c.available).map(channel=>{
    const title=channelPrograms(channel,data.programs).find(p=>p.starts_at<=now&&p.ends_at>now)?.title??'';
    return {channel,title,search:`${channel.number} ${channel.name} ${title}`.toLocaleLowerCase()};
  });
  let page=0;
  const render=()=>{
    const playing=new Set(view.tiles.map(t=>t.playback.id)),query=input.value.trim().toLocaleLowerCase();
    const found=channels.filter(({channel,search})=>(!source.value||channel.source_id===Number(source.value))&&(!provider.value||`${channel.source_id}:${channel.provider_id}`===provider.value)&&(!favorites.checked||data.preferences?.favorites.includes(channel.id))&&search.includes(query))
      .sort((a,b)=>Number(playing.has(a.channel.id))-Number(playing.has(b.channel.id)));
    page=Math.min(page,Math.max(0,Math.ceil(found.length/50)-1));
    dialog.querySelector('.tv-multi-picker-count')!.textContent=`${found.length} channels · ${view.tiles.length} of 4 streams in use`;
    dialog.querySelector('.tv-multi-picker-results')!.innerHTML=found.slice(page*50,page*50+50).map(({channel,title})=>{
      const logo=channelArtwork(channel,data),image=logo?`<img data-tv-image src="${escape(logo)}" data-fallback-src="${escape(channelArtworkFallback(channel,data))}" alt="" referrerpolicy="no-referrer" loading="lazy"/>`:'';
      return `<button type="button" class="tv-player-channel" data-multi-picker-channel="${channel.id}" ${playing.has(channel.id)?'disabled':''}><span class="tv-player-channel-logo">${image}</span><span><strong>${escape(channel.number)} · ${escape(channel.name)}</strong><span>${escape(title)}</span><small>${playing.has(channel.id)?'Already watching':'Add stream'}</small></span></button>`;
    }).join('')||'<p>No channels match. Try another search or source.</p>';
    dialog.querySelector<HTMLButtonElement>('[data-multi-picker-page="-1"]')!.disabled=page===0;
    dialog.querySelector<HTMLButtonElement>('[data-multi-picker-page="1"]')!.disabled=(page+1)*50>=found.length;
  };
  const providers=()=>{
    const options=(data.providers??[]).filter(p=>p.enabled&&(!source.value||p.source_id===Number(source.value))).sort((a,b)=>a.name.localeCompare(b.name));
    provider.innerHTML='<option value="">All providers</option>'+options.map(p=>`<option value="${escape(p.source_id+':'+p.id)}">${escape(p.name)}</option>`).join('');
  };
  providers();render();
  for(const element of [input,provider,favorites])element.addEventListener('input',()=>{page=0;render();},{signal});
  source.addEventListener('change',()=>{providers();page=0;render();},{signal});
  dialog.addEventListener('click',event=>{
    const button=(event.target as Element).closest<HTMLButtonElement>('button');
    if(button?.hasAttribute('data-multi-picker-close')){close();return;}
    if(button?.dataset.multiPickerPage){page+=Number(button.dataset.multiPickerPage);render();dialog.querySelector('.tv-multi-picker-results')!.scrollTop=0;return;}
    const channel=data.channels.find(c=>c.id===Number(button?.dataset.multiPickerChannel));
    if(channel&&state.tvMultiview===view&&state.bootstrap?.current_user?.id===view.owner){close();void add(channel);}
    const box=dialog.getBoundingClientRect();if(event.target===dialog&&(event.clientX<box.left||event.clientX>box.right||event.clientY<box.top||event.clientY>box.bottom))close();
  },{signal});
  dialog.addEventListener('cancel',event=>{event.preventDefault();close();},{signal});
  dialog.addEventListener('error',event=>{
    const image=event.target;if(!(image instanceof HTMLImageElement))return;
    if(image.dataset.fallbackSrc&&image.getAttribute('src')!==image.dataset.fallbackSrc)image.src=image.dataset.fallbackSrc;else image.hidden=true;
  },{capture:true,signal});
  document.body.append(dialog);createIcons({icons,root:dialog});dialog.showModal();input.focus();
}
