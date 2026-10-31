/** Defines the bounded channel row window and exposes offscreen rows to guide navigation. */
import type { LiveTvState } from './liveTvActions';
import { activeGuideView } from './liveTvGuideModel';

/** Fixed guide row geometry shared by spacers, scrolling, and keyboard navigation. */
export const TV_ROW_HEIGHT = 88;
/** The sticky timeline occupies the first 48 pixels of the guide scroller. */
export const TV_HEADER_HEIGHT = 48;
const OVERSCAN = 6;

/** Scroll state is scoped to the current source and channel filters, rather than selection. */
export interface TvGuideViewport { key:string; top:number; height:number }

/** A new source/filter starts at the beginning; time-window changes retain the channel position. */
export function tvGuideViewKey(data: LiveTvState): string {
  return JSON.stringify([data.owner,activeGuideView(data)?.key,data.providerView,data.query,data.group,data.category,data.channelView]);
}

/** Keep a few neighboring rows mounted while preserving the full lineup's scroll height. */
export function tvGuideRowRange(count: number, top = 0, height = 580): {start:number;end:number} {
  if (count <= 40) return {start:0,end:count};
  const visible = Math.max(1,Math.ceil((height-TV_HEADER_HEIGHT)/TV_ROW_HEIGHT)+1);
  const first = Math.min(Math.max(0,count-visible),Math.max(0,Math.floor(top/TV_ROW_HEIGHT)));
  return {start:Math.max(0,first-OVERSCAN),end:Math.min(count,first+visible+OVERSCAN)};
}

type EnsureRow = (index:number) => HTMLElement | undefined;
const viewports = new WeakMap<HTMLElement,EnsureRow>();

/** Register the current scroller without keeping removed pages or accounts alive. */
export function registerTvGuideViewport(guide: HTMLElement, ensure: EnsureRow, signal: AbortSignal): void {
  viewports.set(guide,ensure);
  signal.addEventListener('abort',()=>{if (viewports.get(guide) === ensure) viewports.delete(guide);},{once:true});
}

/** Materialize a row before keyboard/gamepad input moves beyond the mounted window. */
export function ensureTvGuideRow(guide: HTMLElement, index: number): HTMLElement | undefined {
  return viewports.get(guide)?.(index);
}
