/** Mounts only nearby channel rows while preserving scroll geometry, focus, and the player. */
import { createIcons, icons } from 'lucide';
import { state } from './state';
import { setElementHtml } from './domPatcher';
import { visibleGuideChannels } from './liveTvGuideModel';
import { renderTvGuideRows } from './liveTvGuide';
import { registerTvGuideViewport, tvGuideRowRange, tvGuideViewKey, TV_ROW_HEIGHT } from './tvGuideViewport';

/** Refresh channel rows on coalesced scroll/resize events, leaving the hero and timeline mounted. */
export function bindTvGuideViewport(signal: AbortSignal): void {
  const data = state.liveTv;
  const guide = document.querySelector<HTMLElement>('#tv-guide-scroll');
  const rows = guide?.querySelector<HTMLElement>('#tv-guide-rows');
  if (!data || !guide || !rows) return;
  const channels = visibleGuideChannels(data), key = tvGuideViewKey(data);
  guide.scrollTop = data.guideViewport?.key === key ? data.guideViewport.top : 0;
  let pending = 0;
  const update = () => {
    if (signal.aborted || state.liveTv !== data) return;
    data.guideViewport = {key,top:guide.scrollTop,height:guide.clientHeight || 580};
    const range = tvGuideRowRange(channels.length,guide.scrollTop,data.guideViewport.height);
    const stamp = `${range.start}:${range.end}`;
    if (rows.dataset.tvRange === stamp) return;
    const active = document.activeElement as HTMLElement | null;
    const activeRow = active?.closest<HTMLElement>('[data-tv-row-index]');
    if (activeRow && rows.contains(activeRow)) {
      const index = Number(activeRow.dataset.tvRowIndex);
      if (index < range.start || index >= range.end) guide.focus({preventScroll:true});
    }
    setElementHtml(rows,renderTvGuideRows(data,channels));
    rows.dataset.tvRange = stamp;
    createIcons({icons,root:rows});
  };
  const schedule = () => {
    if (signal.aborted || state.liveTv !== data) return;
    data.guideViewport = {key,top:guide.scrollTop,height:guide.clientHeight || 580};
    if (!pending) pending = requestAnimationFrame(()=>{pending=0;update();});
  };
  guide.addEventListener('scroll',schedule,{signal,passive:true});
  const observer = new ResizeObserver(schedule); observer.observe(guide);
  signal.addEventListener('abort',()=>{observer.disconnect();if (pending) cancelAnimationFrame(pending);},{once:true});
  registerTvGuideViewport(guide,index=>{
    if (index < 0 || index >= channels.length) return;
    const id = `#tv-channel-row-${channels[index].id}`;
    if (!rows.querySelector(id)) { guide.scrollTop = index*TV_ROW_HEIGHT; update(); }
    return rows.querySelector<HTMLElement>(id) ?? undefined;
  },signal);
  update();
}
