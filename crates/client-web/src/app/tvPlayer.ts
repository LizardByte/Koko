/** Adds program details and a compact channel guide to the shared full-screen video player. */
import type { TvChannel, TvMetadata } from '../liveTvApi';
import { tvStreamUrl } from '../liveTvApi';
import { state } from './state';
import { escapeHtml as escape } from './format';
import { channelArtwork, channelArtworkFallback, channelPrograms, guideSources, programBadges, tvImage, tvTime } from './liveTvGuideModel';
import { renderIcon } from './ui';
import { setElementHtml } from './domPatcher';

/** Build the same channel artwork and playback details for the main guide and the player guide. */
export function tvChannelPlayback(channel: TvChannel, guideOpen = false): Omit<NonNullable<typeof state.activeTvPlayback>, 'owner'> {
  const data = state.liveTv!, now = Date.now()/1000;
  const program = channelPrograms(channel,data.programs).find(p => p.starts_at <= now && p.ends_at > now);
  return {kind:'channels',id:channel.id,title:channel.name,url:tvStreamUrl('channels',channel.id),guideOpen,
    image:tvImage(program?.metadata.image ?? channel.logo),logo:channelArtwork(channel,data),logoFallback:channelArtworkFallback(channel,data)};
}

function programDetails(program: {title:string;metadata:TvMetadata;starts_at?:number;ends_at?:number}): string {
  const metadata = program.metadata;
  const tags = [...programBadges(metadata),metadata.episode,metadata.rating,...(metadata.categories ?? []).slice(0,3)].filter(Boolean);
  const time = program.starts_at && program.ends_at ? `${tvTime(program.starts_at)} – ${tvTime(program.ends_at)}` : '';
  const subtitle = metadata.subtitle ? `<p class="tv-player-subtitle">${escape(metadata.subtitle)}</p>` : '';
  const description = metadata.description ? `<p class="tv-player-description">${escape(metadata.description)}</p>` : '';
  return `<h3>${escape(program.title)}</h3>${subtitle}<div class="tv-player-meta">${escape([time,...tags].filter(Boolean).join(' · '))}</div>${description}`;
}

/** Describe the playing channel, including time-shifted broadcasts when their listings remain available. */
export function renderTvPlayerProgram(positionMs?: number): string {
  const tv = state.activeTvPlayback, data = state.liveTv;
  if (!tv || !data || data.owner !== tv.owner) return '';
  if (tv.kind === 'recordings') {
    const recording = data.recordings.find(r => r.id === tv.id);
    return recording ? programDetails(recording) : '';
  }
  const channel = data.channels.find(c => c.id === tv.id);
  const lag = tv.buffer && positionMs !== undefined ? Math.max(0,tv.buffer.end_ms-positionMs) : 0;
  const time = (Date.now()-lag)/1000;
  const program = channel && channelPrograms(channel,data.programs).find(p => p.starts_at <= time && p.ends_at > time);
  return program ? programDetails(program) : '<p class="tv-player-meta">No program information available</p>';
}

/** Update changing program details without replacing the video or its controls. */
export function refreshTvPlayerProgram(positionMs?: number): void {
  const details = document.querySelector<HTMLElement>('#player-tv-program');
  if (!details) return;
  const player = document.querySelector<HTMLMediaElement>('#media-player');
  if (positionMs === undefined && player) positionMs = Number(player.dataset.playbackOffsetMs ?? state.activeTvPlayback?.startMs ?? 0)+player.currentTime*1000;
  const markup = renderTvPlayerProgram(positionMs);
  if (details.dataset.content !== markup) { details.innerHTML = markup; details.dataset.content = markup; }
}

function channelImage(channel: TvChannel): string {
  const data = state.liveTv!;
  const logo = channelArtwork(channel,data), fallback = channelArtworkFallback(channel,data);
  const image = logo ? `<img data-tv-image src="${escape(logo)}" data-fallback-src="${escape(fallback)}" alt="" referrerpolicy="no-referrer" loading="lazy" />` : '';
  return `<span class="tv-player-channel-logo">${image}</span>`;
}

function guideRow(channel: TvChannel): string {
  const programs = channelPrograms(channel,state.liveTv!.programs), now = Date.now()/1000;
  const current = programs.find(p => p.starts_at <= now && p.ends_at > now);
  const next = programs.find(p => p.starts_at > now);
  const title = current?.title ?? 'No listings available';
  const nextTitle = next ? `<small>Next · ${escape(tvTime(next.starts_at))} · ${escape(next.title)}</small>` : '';
  const tags = current ? programBadges(current.metadata).join(' · ') : '';
  const flags = tags ? `<small class="tv-player-flags">${escape(tags)}</small>` : '';
  const selected = state.activeTvPlayback?.id === channel.id;
  const search = `${channel.number} ${channel.name} ${title}`.toLocaleLowerCase();
  return `<button id="player-tv-channel-${channel.id}" type="button" class="tv-player-channel" data-player-tv-channel="${channel.id}" data-tv-source-id="${channel.source_id}" data-tv-search="${escape(search)}" aria-pressed="${selected}" aria-label="Watch ${escape(channel.name)} live">
    ${channelImage(channel)}<span><strong>${escape(channel.number)} · ${escape(channel.name)}</strong><span>${escape(title)}</span>${flags}${nextTitle}</span></button>`;
}

function guideRows(): string {
  return state.liveTv?.channels.filter(c => c.enabled && c.available).map(guideRow).join('') ?? '';
}

/** Render a smaller Now/Next guide alongside the playing video. */
export function renderTvPlayerGuide(): string {
  const tv = state.activeTvPlayback, data = state.liveTv;
  if (!tv || !data || tv.kind !== 'channels') return '';
  const sources = guideSources(data).map(source => `<option value="${source.id}">${escape(source.name)}</option>`).join('');
  return `<aside id="player-tv-guide" class="tv-player-guide" aria-label="Channel guide" ${tv.guideOpen ? '' : 'hidden'}>
    <header><h3>Channel guide</h3><button id="player-tv-guide-close" class="player-icon-button" type="button" aria-label="Close channel guide">${renderIcon('x','player-control-icon')}</button></header>
    <label><span class="sr-only">Find a channel</span><input id="player-tv-guide-search" type="search" placeholder="Find a channel" /></label>
    <label><span class="sr-only">Guide source</span><select id="player-tv-guide-source" aria-label="Guide source"><option value="">All sources</option>${sources}</select></label>
    <div id="player-tv-guide-channels" class="tv-player-guide-channels">${guideRows()}</div><p id="player-tv-guide-empty" hidden>No channels match this search.</p></aside>`;
}

function recoverImage(image: HTMLImageElement): void {
  const fallback = image.dataset.fallbackSrc;
  if (fallback && fallback !== image.getAttribute('src')) image.src = fallback;
  else image.hidden = true;
}

function filterChannels(): void {
  const query = document.querySelector<HTMLInputElement>('#player-tv-guide-search')?.value.toLocaleLowerCase() ?? '';
  const source = document.querySelector<HTMLSelectElement>('#player-tv-guide-source')?.value ?? '';
  let count = 0;
  document.querySelectorAll<HTMLElement>('[data-player-tv-channel]').forEach(channel => {
    channel.hidden = Boolean((source && source !== channel.dataset.tvSourceId) || !channel.dataset.tvSearch?.includes(query));
    if (!channel.hidden) count++;
  });
  const empty = document.querySelector<HTMLElement>('#player-tv-guide-empty');
  if (empty) empty.hidden = count > 0;
}

/** Refresh listings only when their content changes, retaining focused channel buttons. */
export function refreshTvPlayerGuide(): void {
  const list = document.querySelector<HTMLElement>('#player-tv-guide-channels');
  if (!list) return;
  const markup = guideRows();
  if (list.dataset.content === markup) return;
  setElementHtml(list,markup); list.dataset.content = markup; filterChannels();
}

function navigateGuide(event: KeyboardEvent, panel: HTMLElement, close: () => void): void {
  event.stopPropagation();
  if (event.key === 'Escape') { event.preventDefault(); close(); return; }
  if ((event.target as Element).matches('input,select')) return;
  if (!['ArrowUp','ArrowDown'].includes(event.key)) return;
  const channels = Array.from(panel.querySelectorAll<HTMLElement>('[data-player-tv-channel]')).filter(channel => !channel.hidden);
  const index = channels.indexOf(document.activeElement as HTMLElement), step = event.key === 'ArrowDown' ? 1 : -1;
  const target = channels[Math.max(0,Math.min(channels.length-1,index+step))];
  target?.focus({preventScroll:true}); target?.scrollIntoView({block:'nearest'}); event.preventDefault();
}

function observePlayerControls(shell: HTMLElement | null): () => void {
  const controls = shell?.querySelector<HTMLElement>('.player-bottom-controls');
  if (!controls || !shell) return () => undefined;
  const measure = () => {
    const height = controls.getBoundingClientRect().height-Number.parseFloat(getComputedStyle(controls).paddingTop)+16;
    shell.style.setProperty('--tv-player-controls-height',`${height}px`);
  };
  const observer = new ResizeObserver(measure);
  observer.observe(controls); measure();
  return () => observer.disconnect();
}

/** Bind independently of page refreshes so browsing channels never interrupts the decoder. */
export function bindTvPlayerGuide(onTune: (channel: TvChannel) => void): () => void {
  const events = new AbortController(), signal = events.signal;
  const root = document.querySelector<HTMLElement>('#media-player-overlay');
  const shell = document.querySelector<HTMLElement>('#media-player-shell');
  const panel = document.querySelector<HTMLElement>('#player-tv-guide');
  const toggle = document.querySelector<HTMLButtonElement>('#player-tv-guide-toggle');
  const cleanupLayout = panel ? observePlayerControls(shell) : () => undefined;
  const setOpen = (open: boolean) => {
    if (!panel || !state.activeTvPlayback) return;
    state.activeTvPlayback.guideOpen = open; panel.hidden = !open;
    shell?.classList.toggle('has-tv-guide',open); toggle?.setAttribute('aria-expanded',String(open));
    if (open) panel.querySelector<HTMLElement>('[data-player-tv-channel][aria-pressed="true"]')?.focus({preventScroll:true});
    else toggle?.focus({preventScroll:true});
  };
  toggle?.addEventListener('click',()=>setOpen(Boolean(panel?.hidden)),{signal});
  document.querySelector('#player-tv-guide-close')?.addEventListener('click',()=>setOpen(false),{signal});
  panel?.addEventListener('keydown',event=>navigateGuide(event,panel,()=>setOpen(false)),{signal});
  panel?.addEventListener('click',event=>{
    const button = (event.target as Element).closest<HTMLElement>('[data-player-tv-channel]');
    const channel = state.liveTv?.channels.find(c => c.id === Number(button?.dataset.playerTvChannel));
    if (channel && channel.id !== state.activeTvPlayback?.id) onTune(channel);
  },{signal});
  document.querySelector('#player-tv-guide-search')?.addEventListener('input',filterChannels,{signal});
  document.querySelector('#player-tv-guide-source')?.addEventListener('change',filterChannels,{signal});
  root?.addEventListener('error',event=>{
    if (event.target instanceof HTMLImageElement && event.target.hasAttribute('data-tv-image')) recoverImage(event.target);
  },{capture:true,signal});
  root?.querySelectorAll<HTMLImageElement>('[data-tv-image]').forEach(image=>{if (image.complete && !image.naturalWidth) recoverImage(image);});
  return () => { events.abort(); cleanupLayout(); };
}
