/** Source selection, category matching, and time-aligned geometry for the TV guide. */
import { resolveApiUrl } from '../api';
import type { TvChannel, TvMetadata, TvProgram, TvStation, TvProvider } from '../liveTvApi';
import type { LiveTvState } from './liveTvActions';

import { TV_WINDOW_SECONDS } from './tvGuideTime';
/** Viewer-facing program categories supported by common XMLTV and tuner guides. */
export const TV_CATEGORIES = ['All programs', 'Sports', 'Movies', 'News', 'Kids', 'Music', 'Documentaries'] as const;
const CATEGORY_PATTERNS: Record<string, RegExp> = {
  Sports: /sport|football|baseball|basketball|soccer|hockey|tennis|racing|golf|wrestling/i,
  Movies: /movie|film|cinema/i, News: /news|current affairs|weather/i,
  Kids: /child|kids|youth|animation|cartoon/i, Music: /music|concert/i, Documentaries: /documentar/i,
};

const schedules = new WeakMap<TvProgram[], {length:number; sources:Map<number,Map<string,TvProgram[]>>}>();
const stationIndexes = new WeakMap<TvStation[], Map<number,Map<string,TvStation>>>();
const channelViews = new WeakMap<LiveTvState, {key:unknown[]; channels:TvChannel[]}>();
const timeFormat = new Intl.DateTimeFormat(undefined,{hour:'numeric',minute:'2-digit'});
const dateFormat = new Intl.DateTimeFormat(undefined,{month:'short',day:'numeric',hour:'numeric',minute:'2-digit'});

/** Match source-scoped station IDs, respecting an explicit channel mapping. */
export function channelPrograms(channel: TvChannel, programs: TvProgram[]): TvProgram[] {
  const key = channel.guide_override ?? channel.guide_key;
  let index = schedules.get(programs);
  if (!index || index.length !== programs.length) {
    const sources = new Map<number,Map<string,TvProgram[]>>();
    for (const program of programs) {
      const stations = sources.get(program.source_id) ?? new Map<string,TvProgram[]>();
      const rows = stations.get(program.guide_key) ?? [];
      rows.push(program); stations.set(program.guide_key,rows); sources.set(program.source_id,stations);
    }
    for (const stations of sources.values()) for (const rows of stations.values()) rows.sort((a,b)=>a.starts_at-b.starts_at);
    index = {length:programs.length,sources}; schedules.set(programs,index);
  }
  return index.sources.get(channel.source_id)?.get(key) ?? [];
}

/** Format UTC seconds in the viewer's local timezone. */
export function tvTime(seconds: number, date = false): string {
  return (date ? dateFormat : timeFormat).format(seconds*1000);
}

/** Accept remote artwork and the authenticated Koko program-artwork endpoint. */
export function tvImage(value?: string | null): string {
  if (!value) return '';
  if (/^\/api\/v1\/live-tv\/(?:programs|channels)\/\d+\/artwork(?:\?kind=poster)?$/.test(value)) return resolveApiUrl(value);
  try {
    const url = new URL(value);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return '';
    if (url.hostname === 'tmsimg.com' || url.hostname.endsWith('.tmsimg.com')) url.searchParams.set('w',url.pathname.includes('/h3/NowShowing/') ? '320' : '960');
    return url.href.replace(/'/g,'%27');
  }
  catch { return ''; }
}

/** Prefer the channel's guide station logo when the tuner supplies no artwork. */
export function channelArtwork(channel: TvChannel, data: LiveTvState): string {
  return tvImage(channel.logo) || channelArtworkFallback(channel,data);
}

/** Guide station artwork can replace an unavailable tuner logo in both guide and player. */
export function channelArtworkFallback(channel: TvChannel, data: LiveTvState): string {
  const key = channel.guide_override ?? channel.guide_key;
  let index = stationIndexes.get(data.stations);
  if (!index) {
    index = new Map();
    for (const station of data.stations) {
      const rows = index.get(station.source_id) ?? new Map<string,TvStation>();
      if (!rows.has(station.guide_key)) rows.set(station.guide_key,station);
      index.set(station.source_id,rows);
    }
    stationIndexes.set(data.stations,index);
  }
  return tvImage(index.get(channel.source_id)?.get(key)?.logo);
}

/** Distinguish actual live broadcasts from programs merely airing at the current time. */
export function programBadges(metadata: TvMetadata): string[] {
  const badges: string[] = [];
  if (metadata.is_live) badges.push('LIVE');
  if (metadata.is_premiere) badges.push('PREMIERE');
  if (metadata.is_finale) badges.push('FINALE');
  if (metadata.is_new) badges.push('NEW');
  return badges;
}

/** Match program categories without guessing from the title or conflating NEW and LIVE. */
export function matchesTvCategory(program: TvProgram, category?: string): boolean {
  return !category || category === 'All programs' || (program.metadata.categories ?? []).some(c => CATEGORY_PATTERNS[category]?.test(c));
}

/** Clip broadcasts to the visible time range while preserving schedule gaps and duration. */
export function programPosition(program: TvProgram, start: number): { left: number; width: number } | undefined {
  const from = Math.max(start, program.starts_at), to = Math.min(start + TV_WINDOW_SECONDS, program.ends_at);
  if (to <= from) return;
  return { left:(from - start) / TV_WINDOW_SECONDS * 100, width:(to - from) / TV_WINDOW_SECONDS * 100 };
}

/** List available sources without exposing server setup or connection credentials. */
export function guideSources(data: LiveTvState): { id: number; name: string; count: number }[] {
  const ids = [...new Set(data.channels.filter(c => c.enabled && c.available).map(c => c.source_id))];
  return ids.map(id => ({ id, name:data.guideSources?.find(s => s.id === id)?.name ?? `TV source ${id}`,
    count:data.channels.filter(c => c.source_id === id && c.enabled && c.available).length }));
}

/** Keep source navigation stable when devices disappear or channel setup changes. */
export function activeGuideSource(data: LiveTvState): number | undefined {
  return activeGuideView(data)?.sourceIds[0];
}

/** One rail entry represents either a named group or an ungrouped source. */
export interface TvGuideView { key:string; name:string; count:number;sourceIds:number[];groupId?:string }

/** Group connected sources without deduplicating channels or combining tuner limits. */
export function guideViews(data: LiveTvState): TvGuideView[] {
  const sources = guideSources(data), counts = new Map(sources.map(s=>[s.id,s.count]));
  const grouped = new Set<number>();
  const views:TvGuideView[] = [];
  for (const group of data.sourceGroups ?? []) {
    const ids = group.source_ids.filter(id=>counts.has(id));
    ids.forEach(id=>grouped.add(id));
    if (ids.length) views.push({key:`group:${group.id}`,groupId:group.id,name:group.name,sourceIds:ids,count:ids.reduce((sum,id)=>sum+(counts.get(id) ?? 0),0)});
  }
  for (const source of sources) if (!grouped.has(source.id)) views.push({key:`source:${source.id}`,name:source.name,count:source.count,sourceIds:[source.id]});
  return views;
}

/** Retain the selected group, including when a previously standalone member is grouped. */
export function activeGuideView(data: LiveTvState): TvGuideView | undefined {
  const views = guideViews(data);
  return views.find(v=>data.sourceGroupId && v.groupId === data.sourceGroupId)
    ?? views.find(v=>!v.groupId && v.sourceIds[0] === data.sourceId)
    ?? views.find(v=>v.sourceIds.includes(data.sourceId ?? -1)) ?? views[0];
}

/** Provider navigation stays within the selected connection or named source group. */
export function guideProviders(data:LiveTvState):TvProvider[] {
  const ids=new Set(activeGuideView(data)?.sourceIds);
  return (data.providers ?? []).filter(p=>p.enabled&&ids.has(p.source_id));
}

/** Ignore a stale provider selection when its connection/group is no longer displayed. */
export function activeGuideProvider(data:LiveTvState):TvProvider|undefined {
  return guideProviders(data).find(p=>`${p.source_id}:${p.id}`===data.providerView);
}

/** Filter within a source while retaining all time positions for the displayed channels. */
export function visibleGuideChannels(data: LiveTvState): TvChannel[] {
  const view = activeGuideView(data), query = (data.query ?? '').toLocaleLowerCase();
  const sourceIds = new Set(view?.sourceIds);
  const favorites = data.preferences?.favorites ?? [], recent = data.preferences?.recent ?? [];
  const provider=activeGuideProvider(data);
  const key = [data.channels,data.channels.length,data.programs,data.programs.length,view?.key,data.sourceGroups,data.providers,data.providerView,query,data.group,data.category,data.channelView,data.windowStart,data.preferences?.favorites,data.preferences?.recent];
  const cached = channelViews.get(data);
  if (cached && key.every((value,i)=>Object.is(value,cached.key[i]))) return cached.channels;
  const favoriteIds = new Set(favorites), recentOrder = new Map(recent.map((id,i)=>[id,i]));
  const channels = data.channels.filter(c => {
    if (!c.enabled || !c.available || !sourceIds.has(c.source_id) || (data.group && c.group_name !== data.group)) return false;
    if (provider&&(c.source_id!==provider.source_id || c.provider_id!==provider.id)) return false;
    if (data.channelView === 'favorites' && !favoriteIds.has(c.id)) return false;
    if (data.channelView === 'recent' && !recentOrder.has(c.id)) return false;
    if (!query && (!data.category || data.category === 'All programs')) return true;
    const programs = channelPrograms(c, data.programs).filter(p => programPosition(p, data.windowStart) && matchesTvCategory(p, data.category));
    if (data.category && data.category !== 'All programs' && !programs.length) return false;
    return !query || `${c.number} ${c.name}`.toLocaleLowerCase().includes(query) || programs.some(p => p.title.toLocaleLowerCase().includes(query));
  });
  if (data.channelView === 'recent') channels.sort((a,b) => recentOrder.get(a.id)!-recentOrder.get(b.id)!);
  channelViews.set(data,{key,channels});
  return channels;
}

/** Resolve the highlighted program for the hero, falling back to the current broadcast. */
export function guideSelection(data: LiveTvState): { channel?: TvChannel; program?: TvProgram } {
  const channels = visibleGuideChannels(data);
  const selectedChannel = channels.find(c => c.id === data.selectedChannelId);
  const now = Date.now() / 1000;
  let first: {channel:TvChannel;program:TvProgram} | undefined;
  for (const channel of selectedChannel ? [selectedChannel] : channels) {
    const rows = channelPrograms(channel,data.programs).filter(p=>programPosition(p,data.windowStart) && matchesTvCategory(p,data.category));
    const selected = rows.find(p=>p.id === data.selectedProgramId);
    if (selected) return {channel,program:selected};
    const current = rows.find(p=>p.starts_at <= now && p.ends_at > now);
    if (current) return {channel,program:current};
    if (!first && rows[0]) first = {channel,program:rows[0]};
  }
  return first ?? {channel:selectedChannel ?? channels[0]};
}
