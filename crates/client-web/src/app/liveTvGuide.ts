/** Renders the TV hero and a single scrollable, time-aligned electronic program guide. */
import type { TvChannel, TvProgram } from '../liveTvApi';
import { escapeHtml as escape } from './format';
import { state } from './state';
import { renderButtonContent, renderIcon } from './ui';
import { renderFeatureHero } from './featureHero';
import { renderPlayerOverlay } from './playbackController';
import type { LiveTvState } from './liveTvActions';
import { activeGuideView, channelArtwork, channelArtworkFallback, channelPrograms, guideSelection, guideViews, matchesTvCategory, programBadges, programPosition, tvImage, tvTime, TV_CATEGORIES, visibleGuideChannels } from './liveTvGuideModel';
import { currentGuideWindow, TV_SLOT_SECONDS } from './tvGuideTime';
import { renderMetadataSearchProviderAttribution } from './providers';
import { tvGuideRowRange, tvGuideViewKey, TV_ROW_HEIGHT } from './tvGuideViewport';
import { activeGuideProvider,guideProviders } from './liveTvGuideModel';
import { renderTvProviderIcon,renderTvSourceIcon,renderTvAllProvidersIcon } from './liveTvIcons';

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
  return `<button type="button" class="secondary-button home-feature-action" data-tv-record-options="${channel.id}" data-tv-record-program="${program.id}" aria-haspopup="dialog">${renderButtonContent('Record program or series','plus')}</button>`;
}

function heroPlaybackActions(preview: boolean): string {
  if (!preview) return '';
  return `<button type="button" class="secondary-button home-feature-action" id="expand-tv-player">${renderButtonContent('Expand player','maximize')}</button><button type="button" class="secondary-button home-feature-action" id="close-player" aria-label="Stop playback">${renderButtonContent('Stop playback','x')}</button>`;
}

function artworkAttribution(program: TvProgram | undefined): string {
  const providers = program?.metadata.artwork_sources ?? [];
  if (!providers.length) return '';
  return `<div class="tv-artwork-attribution">Artwork providers: ${providers.map(renderMetadataSearchProviderAttribution).join(' ')}</div>`;
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
      <p>${escape(description)}</p><div class="hero-meta-row">${program ? badges(program) : ''}${tags}${timeMarkup}</div>${artworkAttribution(program)}`,
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
  return `<button id="tv-program-${channel.id}-${program.id}" type="button" class="tv-guide-program${onAir ? ' is-on-air' : ''}${dimmed ? ' is-dimmed' : ''}" data-tv-guide-activate data-tv-select-program="${program.id}" data-tv-select-channel="${channel.id}" data-tv-start="${program.starts_at}" data-tv-end="${program.ends_at}" aria-haspopup="dialog" aria-pressed="${selected}" aria-label="${escape(program.title)}, ${escape(tvTime(program.starts_at))} to ${escape(tvTime(program.ends_at))}. Watch ${escape(channel.name)} live; hold for options" style="left:${position.left}%;width:${position.width}%">
    <strong>${escape(program.title)}</strong>${subtitle}<span class="tv-guide-badges">${badges(program)}</span></button>`;
}

function channelRow(channel: TvChannel, data: LiveTvState, index: number): string {
  const favorite = data.preferences?.favorites.includes(channel.id) === true;
  const programs = channelPrograms(channel, data.programs).map(p => programCell(channel, p, data)).join('');
  return `<div id="tv-channel-row-${channel.id}" class="tv-guide-row" data-tv-row-index="${index}" role="row" aria-rowindex="${index+2}"><div class="tv-guide-channel" role="rowheader"><button id="tv-channel-select-${channel.id}" type="button" class="tv-channel-select" data-tv-guide-activate data-tv-select-channel="${channel.id}" aria-haspopup="dialog" aria-label="Watch ${escape(channel.name)} live; hold for options">
    <span class="tv-channel-logo-slot" aria-hidden="true">${channelLogo(channel,data)}</span><span><strong>${escape(channel.number)}</strong><span>${escape(channel.name)}</span></span></button>
    <div class="tv-channel-actions"><button type="button" data-tv-play="${channel.id}" aria-label="Watch ${escape(channel.name)} live">${renderIcon('play')}</button>
    <button type="button" data-tv-favorite="${channel.id}" aria-pressed="${favorite}" aria-label="${favorite ? 'Remove' : 'Add'} ${escape(channel.name)} ${favorite ? 'from' : 'to'} favorites">${renderIcon('star')}</button></div></div>
    <div class="tv-guide-track" role="cell">${programs || '<span class="tv-guide-no-listings">No listings available · Watch this channel live</span>'}</div></div>`;
}

/** Render only the viewport's channel rows, with spacers for the remainder of the lineup. */
export function renderTvGuideRows(data: LiveTvState, channels = visibleGuideChannels(data)): string {
  const viewport = data.guideViewport?.key === tvGuideViewKey(data) ? data.guideViewport : undefined;
  const {start,end} = tvGuideRowRange(channels.length,viewport?.top,viewport?.height);
  const selection = guideSelection(data);
  const selected = {...data,selectedProgramId:selection.program?.id,selectedChannelId:selection.channel?.id};
  return `<div id="tv-guide-spacer-top" role="presentation" aria-hidden="true" style="height:${start*TV_ROW_HEIGHT}px"></div>${channels.slice(start,end).map((c,i)=>channelRow(c,selected,start+i)).join('')}<div id="tv-guide-spacer-bottom" role="presentation" aria-hidden="true" style="height:${(channels.length-end)*TV_ROW_HEIGHT}px"></div>`;
}

/** Align channel rows beneath one sticky time header and one shared horizontal scrollbar. */
export function renderTvGuide(data: LiveTvState): string {
  const sources = guideViews(data), source = activeGuideView(data), channels = visibleGuideChannels(data);
  const ids = new Set(source?.sourceIds);
  const groups = [...new Set(data.channels.filter(c => ids.has(c.source_id)).map(c => c.group_name).filter(Boolean))].sort((a,b) => a.localeCompare(b));
  const providers=guideProviders(data),provider=activeGuideProvider(data);
  const providerButtons=providers.map(p=>{
    const key=p.source_id+':'+p.id,count=data.channels.filter(c=>c.source_id===p.source_id&&c.provider_id===p.id&&c.enabled&&c.available).length;
    const label=`${p.name}, ${count} ${count===1?'channel':'channels'}`;
    return `<button type="button" data-tv-provider="${escape(key)}" title="${escape(label)}" aria-label="${escape(label)}" aria-pressed="${provider===p}">${renderTvProviderIcon(p)}<span class="tv-source-count" aria-hidden="true">${count}</span></button>`;
  }).join('');
  const providerTabs=providers.length?'<nav class="tv-provider-tabs" aria-label="IPTV providers"><span class="tv-provider-label">Providers</span><button type="button" data-tv-provider="" title="All providers" aria-label="All providers, '+(source?.count??0)+' channels" aria-pressed="'+String(!provider)+'">'+renderTvAllProvidersIcon()+'<span class="tv-source-count" aria-hidden="true">'+(source?.count??0)+'</span></button>'+providerButtons+'</nav>':'';
  const sourceTabs = sources.map(s => {
    const attribute = s.groupId ? 'data-tv-source-group="'+escape(s.groupId)+'"' : 'data-tv-source="'+s.sourceIds[0]+'"';
    const connection=data.guideSources?.find(source=>source.id===s.sourceIds[0]);
    const children=s.key===source?.key?providerTabs:'';
    const label=`${s.name}, ${s.count} ${s.count===1?'channel':'channels'}`;
    return `<button type="button" class="tv-source-tab" ${attribute} title="${escape(label)}" aria-label="${escape(label)}" aria-pressed="${s.key === source?.key}">${renderTvSourceIcon(connection ?? {kind:'tv'})}<span class="tv-source-count" aria-hidden="true">${s.count}</span></button>${children}`;
  }).join('');
  const hasRail=sources.length>1||providers.length>0;
  const connection=data.guideSources?.find(s=>s.id===source?.sourceIds[0]);
  const lineup=!hasRail&&source?'<div class="tv-guide-lineup">'+renderTvSourceIcon(connection ?? {kind:'tv'})+'<strong>'+escape(source.name)+'</strong></div>':'';
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
    ${lineup}<div class="tv-guide-layout${hasRail ? ' has-source-rail' : ''}">${hasRail ? `<nav class="tv-source-rail" aria-label="TV sources"><span class="tv-source-label">Sources</span>${sourceTabs}</nav>` : ''}
    <div class="tv-guide-board">${channels.length ? `<div id="tv-guide-scroll" class="tv-guide-scroll table-shell" data-tv-guide-source="${data.owner}:${escape(source?.key ?? '')}" data-tv-view="${escape(tvGuideViewKey(data))}" data-tv-channel-count="${channels.length}" data-tv-window-start="${data.windowStart}" role="region" aria-label="TV program guide" tabindex="0"><div class="tv-guide-grid" role="table" aria-rowcount="${channels.length+1}" aria-label="${escape(source?.name ?? 'TV')} schedules">
      <div class="tv-guide-time-row" role="row" aria-rowindex="1"><div class="tv-guide-corner" role="columnheader">Channels</div><div class="tv-guide-time-track">${ticks}</div></div><div id="tv-guide-rows" role="rowgroup">${renderTvGuideRows(data,channels)}</div>${nowLine}</div></div>`
      : '<p class="tv-guide-empty">No channels match this view. Try All channels or All programs, or add a TV source in Settings → Live TV.</p>'}
      <p class="tv-guide-hint">Click or press Enter to watch. Hold or right-click for channel and recording options.</p>${data.category && data.category !== 'All programs' ? '<p class="tv-guide-hint">Channels with matching programs are shown. Other programs are dimmed to preserve the schedule.</p>' : ''}</div></div>`;
}
