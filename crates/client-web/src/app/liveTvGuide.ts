/** Renders the TV hero and a single scrollable, time-aligned electronic program guide. */
import type { TvChannel, TvProgram } from '../liveTvApi';
import { escapeHtml as escape } from './format';
import { state } from './state';
import { renderButtonContent, renderIcon } from './ui';
import { renderFeatureHero } from './featureHero';
import { renderPlayerOverlay } from './playbackController';
import type { LiveTvState } from './liveTvActions';
import { activeGuideSource, channelArtwork, channelArtworkFallback, channelPrograms, guideSelection, guideSources, matchesTvCategory, programBadges, programPosition, tvImage, tvTime, TV_CATEGORIES, visibleGuideChannels } from './liveTvGuideModel';
import { currentGuideWindow, TV_SLOT_SECONDS } from './tvGuideTime';

function badges(program: TvProgram): string {
  return programBadges(program.metadata).map(tag => `<span class="tv-program-badge tv-badge-${tag.toLowerCase()}">${tag}</span>`).join('');
}

function channelLogo(channel: TvChannel, data: LiveTvState, hero = false): string {
  const logo = channelArtwork(channel,data);
  if (!logo) return '';
  const fallback = channelArtworkFallback(channel,data);
  return `<img class="${hero ? 'tv-hero-channel-logo' : 'tv-channel-logo'}" data-tv-image data-fallback-src="${escape(fallback)}" src="${escape(logo)}" alt="${hero ? escape(channel.name)+' logo' : ''}" referrerpolicy="no-referrer" loading="lazy" />`;
}

function recordingChoices(channel: TvChannel, program: TvProgram): string {
  if (state.liveTv?.canRecord === false) return '<p>Recording is disabled for your account.</p>';
  return `<details class="tv-record-options"><summary>${renderIcon('plus')} Record program or series</summary><form data-tv-program-record="${program.id}" data-tv-channel="${channel.id}" class="live-tv-record-choices">
    <label>Before (minutes)<input name="before" type="number" min="0" max="60" value="1" /></label><label>After (minutes)<input name="after" type="number" min="0" max="60" value="2" /></label>
    <label><input type="checkbox" name="new-only" /> New episodes only (series)</label><label><input type="checkbox" name="private" /> Private (only my account)</label>
    <button type="submit" name="mode" value="single">Record program</button><button type="submit" name="mode" value="series" class="secondary-button">Record series</button></form></details>`;
}

function heroPlaybackActions(preview: boolean): string {
  if (!preview) return '';
  return `<button type="button" class="secondary-button home-feature-action" id="expand-tv-player">${renderButtonContent('Expand player','maximize')}</button><button type="button" class="secondary-button home-feature-action" id="close-player" aria-label="Stop playback">${renderButtonContent('Stop playback','x')}</button>`;
}

/** Display program artwork and descriptions, reserving the artwork area for minimized playback. */
export function renderTvHero(data: LiveTvState): string {
  const { channel, program } = guideSelection(data);
  const fallbackImage = channel ? channelArtwork(channel,data) : '';
  const image = tvImage(program?.metadata.image) || fallbackImage;
  const poster = tvImage(program?.metadata.poster) || tvImage(program?.metadata.image);
  const now = Date.now() / 1000;
  const onAir = program && program.starts_at <= now && program.ends_at > now;
  const label = onAir ? 'On air now' : 'Program guide';
  const title = program?.title ?? channel?.name ?? 'Live TV';
  const description = program?.metadata.description ?? 'Choose a channel or program to see its details. Channels without listings can still be watched live.';
  const time = program ? `${tvTime(program.starts_at, true)} – ${tvTime(program.ends_at)} · ${Math.round((program.ends_at - program.starts_at)/60)} min` : '';
  const tags = [...(program?.metadata.categories ?? []), program?.metadata.rating, program?.metadata.episode].filter(Boolean).map(tag => `<span class="tag">${escape(tag!)}</span>`).join('');
  const channelLabel = channel ? ` · ${escape(channel.number)} ${escape(channel.name)}` : '';
  const subtitle = program?.metadata.subtitle ? `<p class="home-feature-context">${escape(program.metadata.subtitle)}</p>` : '';
  const watch = channel ? `<button type="button" class="secondary-button home-feature-action" data-tv-play="${channel.id}">${renderButtonContent('Watch channel live','play')}</button>` : '';
  const timeMarkup = time ? `<span class="tag">${escape(time)}</span>` : '';
  const preview = Boolean(state.activeTvPlayback && !state.isPlayerOpen);
  const playbackActions = heroPlaybackActions(preview);
  const logo = channel ? channelLogo(channel,data,true) : '';
  return renderFeatureHero({id:'tv-guide-hero',label:'Program details',image,
    poster:poster ? {url:poster,label:`${title} poster`} : undefined,
    copy:`<div class="tv-hero-channel">${logo}<p class="eyebrow">${label}${channelLabel}</p></div><h2>${escape(title)}</h2>${subtitle}
      <p>${escape(description)}</p><div class="hero-meta-row">${program ? badges(program) : ''}${tags}${timeMarkup}</div>`,
    actions:`<div class="home-feature-actions">${playbackActions}${watch}${program && channel && program.ends_at > now ? recordingChoices(channel,program) : ''}</div>`,
    media:{id:'tv-hero-media',active:preview,content:state.activeTvPlayback ? renderPlayerOverlay() : ''},
  });
}

function programCell(channel: TvChannel, program: TvProgram, data: LiveTvState): string {
  const position = programPosition(program, data.windowStart);
  if (!position) return '';
  const now = Date.now()/1000, onAir = program.starts_at <= now && program.ends_at > now;
  const selected = data.selectedProgramId === program.id && data.selectedChannelId === channel.id;
  const dimmed = !matchesTvCategory(program, data.category);
  const subtitle = program.metadata.subtitle ? `<span class="tv-guide-subtitle">${escape(program.metadata.subtitle)}</span>` : '';
  return `<button id="tv-program-${channel.id}-${program.id}" type="button" class="tv-guide-program${onAir ? ' is-on-air' : ''}${dimmed ? ' is-dimmed' : ''}" data-tv-select-program="${program.id}" data-tv-select-channel="${channel.id}" data-tv-start="${program.starts_at}" data-tv-end="${program.ends_at}" aria-pressed="${selected}" aria-label="${escape(program.title)}, ${escape(tvTime(program.starts_at))} to ${escape(tvTime(program.ends_at))}" style="left:${position.left}%;width:${position.width}%">
    <strong>${escape(program.title)}</strong>${subtitle}<span class="tv-guide-badges">${badges(program)}</span></button>`;
}

function channelRow(channel: TvChannel, data: LiveTvState): string {
  const favorite = data.preferences?.favorites.includes(channel.id) === true;
  const programs = channelPrograms(channel, data.programs).map(p => programCell(channel, p, data)).join('');
  return `<div class="tv-guide-row" role="row"><div class="tv-guide-channel" role="rowheader"><button type="button" class="tv-channel-select" data-tv-select-channel="${channel.id}" aria-label="Details for ${escape(channel.name)}">
    <span class="tv-channel-logo-slot" aria-hidden="true">${channelLogo(channel,data)}</span><span><strong>${escape(channel.number)}</strong><span>${escape(channel.name)}</span></span></button>
    <div class="tv-channel-actions"><button type="button" data-tv-play="${channel.id}" aria-label="Watch ${escape(channel.name)} live">${renderIcon('play')}</button>
    <button type="button" data-tv-favorite="${channel.id}" aria-pressed="${favorite}" aria-label="${favorite ? 'Remove' : 'Add'} ${escape(channel.name)} ${favorite ? 'from' : 'to'} favorites">${renderIcon('star')}</button></div></div>
    <div class="tv-guide-track" role="cell">${programs || '<span class="tv-guide-no-listings">No listings available · Watch this channel live</span>'}</div></div>`;
}

/** Align channel rows beneath one sticky time header and one shared horizontal scrollbar. */
export function renderTvGuide(data: LiveTvState): string {
  const sources = guideSources(data), source = activeGuideSource(data), channels = visibleGuideChannels(data);
  const selection = guideSelection(data);
  const selectedData = {...data,selectedProgramId:selection.program?.id,selectedChannelId:selection.channel?.id};
  const groups = [...new Set(data.channels.filter(c => c.source_id === source).map(c => c.group_name).filter(Boolean))].sort((a,b) => a.localeCompare(b));
  const sourceTabs = sources.map(s => `<button type="button" data-tv-source="${s.id}" aria-pressed="${s.id === source}"><strong>${escape(s.name)}</strong><small>${s.count} ${s.count === 1 ? 'channel' : 'channels'}</small></button>`).join('');
  const categories = TV_CATEGORIES.map(category => `<button type="button" data-tv-category="${category}" aria-pressed="${category === (data.category ?? 'All programs')}">${category}</button>`).join('');
  const ticks = Array.from({length:12}, (_,i) => `<div class="tv-guide-tick" role="columnheader">${escape(tvTime(data.windowStart + i*TV_SLOT_SECONDS))}</div>`).join('');
  const now = (Date.now()/1000 - data.windowStart) / (6*3600) * 100;
  const nowLine = now >= 0 && now < 100 ? `<div class="tv-guide-now" aria-hidden="true" style="left:calc(var(--tv-channel-width) + ${now / 100} * var(--tv-timeline-width))"><span>Now</span></div>` : '';
  const views = [['all','All channels'],['favorites','Favorites'],['recent','Recently played']].map(([key,label]) => `<button type="button" data-tv-channel-view="${key}" aria-pressed="${key === (data.channelView ?? 'all')}">${label}</button>`).join('');
  const filterCount = [data.query,data.group,data.category && data.category !== 'All programs',data.channelView && data.channelView !== 'all'].filter(Boolean).length;
  const filterLabel = filterCount ? `Filters · ${filterCount} active` : 'Filters';
  return `<div class="tv-guide-toolbar"><div class="live-tv-actions"><button type="button" class="secondary-button" data-tv-window="-1" aria-label="Earlier programs" ${data.windowStart <= currentGuideWindow() ? 'disabled' : ''}>${renderIcon('chevron-left')}</button><button type="button" class="secondary-button" data-tv-window="0">Now</button><button type="button" class="secondary-button" data-tv-window="1" aria-label="Later programs">${renderIcon('chevron-right')}</button><strong>${escape(tvTime(data.windowStart,true))}</strong></div>
    <button type="button" id="toggle-tv-filters" class="secondary-button browse-filter-toggle" aria-expanded="${Boolean(data.filtersOpen)}" aria-controls="tv-filter-panel">${renderButtonContent(filterLabel,'sliders-horizontal')}</button></div>
    <div id="tv-filter-panel" ${data.filtersOpen ? '' : 'hidden'}><form id="tv-filter-form"><label><span class="sr-only">Search channels or programs</span><input type="search" name="query" value="${escape(data.query ?? '')}" placeholder="Find channels or programs" /></label>
    <label><span class="sr-only">Channel group</span><select name="group"><option value="">All channel groups</option>${groups.map(g => `<option ${data.group === g ? 'selected' : ''}>${escape(g)}</option>`).join('')}</select></label><button type="submit" class="secondary-button">Search</button></form>
    <nav class="tv-guide-filters" aria-label="Program categories">${categories}</nav><nav class="tv-guide-views" aria-label="Channel lists">${views}</nav></div>
    <div class="tv-guide-layout${sources.length > 1 ? ' has-source-rail' : ''}">${sources.length > 1 ? `<nav class="tv-source-rail" aria-label="TV sources">${sourceTabs}</nav>` : ''}
    <div class="tv-guide-board">${channels.length ? `<div id="tv-guide-scroll" class="tv-guide-scroll table-shell" data-tv-guide-source="${data.owner}:${source}" data-tv-window-start="${data.windowStart}" role="region" aria-label="TV program guide" tabindex="0"><div class="tv-guide-grid" role="table" aria-label="${escape(sources.find(s => s.id === source)?.name ?? 'TV')} schedules">
      <div class="tv-guide-time-row" role="row"><div class="tv-guide-corner" role="columnheader">Channels</div><div class="tv-guide-time-track">${ticks}</div></div>${channels.map(c => channelRow(c,selectedData)).join('')}${nowLine}</div></div>`
      : '<p class="tv-guide-empty">No channels match this view. Try All channels or All programs, or add a TV source in Settings → Live TV.</p>'}
      ${data.category && data.category !== 'All programs' ? '<p class="tv-guide-hint">Channels with matching programs are shown. Other programs are dimmed to preserve the schedule.</p>' : ''}</div></div>`;
}
