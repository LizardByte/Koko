/** Binds guide selection, source navigation, categories, favorites, and recently played views. */
import { setTvFavorite } from '../liveTvApi';
import { state } from './state';

/** Update the guide without restarting a playing or minimized TV stream. */
export function bindTvGuideActions(signal: AbortSignal, context: {render: () => void}, run: (operation: () => Promise<unknown>, button?: HTMLButtonElement) => Promise<void>): void {
  document.querySelectorAll<HTMLImageElement>('[data-tv-image], #tv-guide-hero .home-feature-poster').forEach(image=>{
    const fallback = () => {
      const url = image.dataset.fallbackSrc;
      if (url && image.getAttribute('src') !== url) image.src = url;
      else image.hidden = true;
    };
    image.addEventListener('error',fallback,{signal});
    if (image.complete && !image.naturalWidth) fallback();
  });
  document.querySelector('#toggle-tv-filters')?.addEventListener('click',()=>{
    if (state.liveTv) {state.liveTv.filtersOpen = !state.liveTv.filtersOpen; context.render();}
  },{signal});
  document.querySelectorAll<HTMLButtonElement>('[data-tv-source], [data-tv-category], [data-tv-channel-view]').forEach(button => button.addEventListener('click', () => {
    const data = state.liveTv; if (!data) return;
    if (button.dataset.tvSource) { data.sourceId = Number(button.dataset.tvSource); data.group = undefined; }
    if (button.dataset.tvCategory) data.category = button.dataset.tvCategory;
    if (button.dataset.tvChannelView) data.channelView = button.dataset.tvChannelView as 'all' | 'favorites' | 'recent';
    data.selectedProgramId = undefined; data.selectedChannelId = undefined;
    context.render();
  }, {signal}));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-select-channel]').forEach(button => {
    const select = () => {
      const data = state.liveTv; if (!data) return;
      const channel = Number(button.dataset.tvSelectChannel), program = Number(button.dataset.tvSelectProgram) || undefined;
      if (data.selectedChannelId === channel && data.selectedProgramId === program) return;
      data.selectedChannelId = channel; data.selectedProgramId = program; context.render();
    };
    button.addEventListener('click', select, {signal});
    button.addEventListener('focus', select, {signal});
  });
  document.querySelectorAll<HTMLButtonElement>('[data-tv-favorite]').forEach(button => button.addEventListener('click', () => {
    const id = Number(button.dataset.tvFavorite);
    void run(() => setTvFavorite(id, !state.liveTv?.preferences?.favorites.includes(id)), button);
  }, {signal}));
}
