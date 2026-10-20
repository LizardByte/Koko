/** Positions audio menus, reveals panel entries, and supplies browser media metadata. */

import { getArtworkUrl, type MediaItemSummary } from '../api';

/** Share the same album cover with the dock and OS/browser media controls. */
export function musicMediaMetadata(item: MediaItemSummary): MediaMetadataInit {
  return { title:item.audiobook?.title || item.display_title, artist:item.audiobook?.author ?? item.music?.artist ?? item.display_subtitle ?? '', album:item.audiobook?.title ?? item.music?.album ?? '',
    artwork:[{src:new URL(getArtworkUrl(item.artwork_item_id ?? item.parent_id ?? item.id,'poster',item.artwork_updated_at),location.origin).href}] };
}

export function boundedMenuPosition(trigger: Pick<DOMRect,'right'|'top'|'bottom'>, width:number,height:number,viewportWidth:number,viewportBottom:number):{left:number;top:number} {
  const left=Math.max(12,Math.min(viewportWidth-width-12,trigger.right-width));
  const above=trigger.top-height-8;
  const top=above>=12 ? above : Math.min(trigger.bottom+8,viewportBottom-height-12);
  return {left,top:Math.max(12,Math.min(top,viewportBottom-height-12))};
}

/** Reposition after rendering too: DOM patches remove transient inline placement styles. */
export function bindSongMenus(signal:AbortSignal):void {
  const position=():void=>document.querySelectorAll<HTMLElement>('.music-song-menu [popover]:popover-open').forEach(menu=>{
    const trigger=menu.parentElement?.querySelector('button')?.getBoundingClientRect();
    if(!trigger)return;
    const viewport=globalThis.visualViewport;
    const bottom=Math.min(viewport?.height ?? innerHeight,document.querySelector('.music-dock')?.getBoundingClientRect().top ?? innerHeight);
    const placement=boundedMenuPosition(trigger,menu.offsetWidth,menu.offsetHeight,viewport?.width ?? innerWidth,bottom);
    menu.style.left=`${placement.left}px`; menu.style.top=`${placement.top}px`; menu.style.right='auto';
  });
  document.querySelectorAll<HTMLElement>('.music-song-menu [popover]').forEach(menu=>menu.addEventListener('toggle',position,{signal}));
  globalThis.addEventListener('resize',position,{signal});
  document.addEventListener('scroll',position,{signal,capture:true});
  requestAnimationFrame(()=>{if(!signal.aborted)position();});
}

/** Keep scrolling inside the panel; never scroll the underlying library page. */
export function revealPanelEntry(container:HTMLElement|undefined|null,entry:HTMLElement|undefined|null):void {
  if(!container || !entry)return;
  const parent=container.getBoundingClientRect(),row=entry.getBoundingClientRect();
  container.scrollTop+=row.top-parent.top-container.clientHeight/2+row.height/2;
}
