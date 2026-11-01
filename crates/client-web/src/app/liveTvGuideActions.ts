/** Binds guide selection, source navigation, categories, favorites, and recently played views. */
import { setTvFavorite } from '../liveTvApi';
import { state } from './state';
import { guideSelection } from './liveTvGuideModel';
import { recoverTvSourceImage } from './liveTvIcons';

/** Update the guide without restarting a playing or minimized TV stream. */
export function bindTvGuideActions(signal: AbortSignal, context: {render: () => void;renderTvSelection?:()=>void}, run: (operation: () => Promise<unknown>, button?: HTMLButtonElement) => Promise<void>): void {
  const recoverImage = (image: HTMLImageElement) => {
    if (image.closest('#media-player-overlay')) return;
    const fallback = () => {
      const url = image.dataset.fallbackSrc;
      if (url && image.getAttribute('src') !== url) image.src = url;
      else image.hidden = true;
    };
    fallback();
  };
  document.addEventListener('error',event=>{
    const image = event.target as HTMLImageElement;
    if (image.matches?.('[data-tv-source-image]')) recoverTvSourceImage(image);
    else if (image.matches?.('[data-tv-image], #tv-guide-hero .home-feature-poster')) recoverImage(image);
  },{signal,capture:true});
  document.querySelectorAll<HTMLImageElement>('[data-tv-image], #tv-guide-hero .home-feature-poster').forEach(image=>{
    if (image.complete && !image.naturalWidth) recoverImage(image);
  });
  document.querySelectorAll<HTMLImageElement>('[data-tv-source-image]').forEach(image=>{
    if(image.complete&&!image.naturalWidth) recoverTvSourceImage(image);
  });
  document.querySelector('#toggle-tv-filters')?.addEventListener('click',()=>{
    if (state.liveTv) {state.liveTv.filtersOpen = !state.liveTv.filtersOpen; context.render();}
  },{signal});
  document.querySelectorAll<HTMLButtonElement>('[data-tv-provider]').forEach(button=>button.addEventListener('click',()=>{
    const data=state.liveTv;if(!data){return;}data.providerView=button.dataset.tvProvider || undefined;data.selectedChannelId=undefined;data.selectedProgramId=undefined;data.group=undefined;context.render();
  },{signal}));
  document.querySelectorAll<HTMLButtonElement>('[data-tv-source], [data-tv-source-group], [data-tv-category], [data-tv-channel-view]').forEach(button => button.addEventListener('click', () => {
    const data = state.liveTv; if (!data) return;
    if (button.dataset.tvSource) { data.sourceId = Number(button.dataset.tvSource); data.sourceGroupId = undefined; data.group = undefined;data.providerView=undefined; }
    if (button.dataset.tvSourceGroup) { data.sourceGroupId = button.dataset.tvSourceGroup; data.sourceId = undefined; data.group = undefined;data.providerView=undefined; }
    if (button.dataset.tvCategory) data.category = button.dataset.tvCategory;
    if (button.dataset.tvChannelView) data.channelView = button.dataset.tvChannelView as 'all' | 'favorites' | 'recent';
    data.selectedProgramId = undefined; data.selectedChannelId = undefined;
    context.render();
  }, {signal}));
  const select = (event: Event) => {
      const button = (event.target as Element | null)?.closest<HTMLElement>('[data-tv-select-channel]');
      if (!button) return;
      const data = state.liveTv; if (!data) return;
      const channel = Number(button.dataset.tvSelectChannel), program = Number(button.dataset.tvSelectProgram) || undefined;
      if (data.selectedChannelId === channel && data.selectedProgramId === program) return;
      data.selectedChannelId = channel; data.selectedProgramId = program;
      const selection = guideSelection(data);
      document.querySelectorAll<HTMLElement>('.tv-guide-program').forEach(cell=>cell.setAttribute('aria-pressed',String(Number(cell.dataset.tvSelectChannel) === selection.channel?.id && Number(cell.dataset.tvSelectProgram) === selection.program?.id)));
      if (context.renderTvSelection) context.renderTvSelection(); else context.render();
  };
  document.addEventListener('focusin',select,{signal});
  document.addEventListener('click', event => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>('[data-tv-favorite]');
    if (!button) return;
    const id = Number(button.dataset.tvFavorite);
    void run(() => setTvFavorite(id, !state.liveTv?.preferences?.favorites.includes(id)), button);
  }, {signal});
}
