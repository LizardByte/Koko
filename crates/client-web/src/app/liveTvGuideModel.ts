/** Source selection, category matching, and time-aligned geometry for the TV guide. */
import { resolveApiUrl } from '../api';
import type { TvChannel, TvMetadata, TvProgram } from '../liveTvApi';
import type { LiveTvState } from './liveTvActions';

/** Six hours share one horizontal timeline; each half hour occupies a fixed column. */
export const TV_WINDOW_SECONDS = 6 * 3600;
/** Viewer-facing program categories supported by common XMLTV and tuner guides. */
export const TV_CATEGORIES = ['All programs', 'Sports', 'Movies', 'News', 'Kids', 'Music', 'Documentaries'] as const;
const CATEGORY_PATTERNS: Record<string, RegExp> = {
  Sports: /sport|football|baseball|basketball|soccer|hockey|tennis|racing|golf|wrestling/i,
  Movies: /movie|film|cinema/i, News: /news|current affairs|weather/i,
  Kids: /child|kids|youth|animation|cartoon/i, Music: /music|concert/i, Documentaries: /documentar/i,
};

/** Match source-scoped station IDs, respecting an explicit channel mapping. */
export function channelPrograms(channel: TvChannel, programs: TvProgram[]): TvProgram[] {
  const key = channel.guide_override ?? channel.guide_key;
  return programs.filter(p => p.source_id === channel.source_id && p.guide_key === key).sort((a,b) => a.starts_at - b.starts_at);
}

/** Format UTC seconds in the viewer's local timezone. */
export function tvTime(seconds: number, date = false): string {
  return new Date(seconds * 1000).toLocaleString(undefined, date
    ? { month:'short', day:'numeric', hour:'numeric', minute:'2-digit' } : { hour:'numeric', minute:'2-digit' });
}

/** Accept remote artwork and the authenticated Koko program-artwork endpoint. */
export function tvImage(value?: string | null): string {
  if (!value) return '';
  if (/^\/api\/v1\/live-tv\/(?:programs|channels)\/\d+\/artwork$/.test(value)) return resolveApiUrl(value);
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
  const key = channel.guide_override ?? channel.guide_key;
  return tvImage(channel.logo) || tvImage(data.stations.find(s => s.source_id === channel.source_id && s.guide_key === key)?.logo);
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
  const sources = guideSources(data);
  return sources.find(s => s.id === data.sourceId)?.id ?? sources[0]?.id;
}

/** Filter within a source while retaining all time positions for the displayed channels. */
export function visibleGuideChannels(data: LiveTvState): TvChannel[] {
  const source = activeGuideSource(data), query = (data.query ?? '').toLocaleLowerCase();
  const favorites = data.preferences?.favorites ?? [], recent = data.preferences?.recent ?? [];
  const channels = data.channels.filter(c => {
    if (!c.enabled || !c.available || c.source_id !== source || (data.group && c.group_name !== data.group)) return false;
    if (data.channelView === 'favorites' && !favorites.includes(c.id)) return false;
    if (data.channelView === 'recent' && !recent.includes(c.id)) return false;
    const programs = channelPrograms(c, data.programs).filter(p => programPosition(p, data.windowStart) && matchesTvCategory(p, data.category));
    if (data.category && data.category !== 'All programs' && !programs.length) return false;
    return !query || `${c.number} ${c.name}`.toLocaleLowerCase().includes(query) || programs.some(p => p.title.toLocaleLowerCase().includes(query));
  });
  return data.channelView === 'recent' ? channels.sort((a,b) => recent.indexOf(a.id) - recent.indexOf(b.id)) : channels;
}

/** Resolve the highlighted program for the hero, falling back to the current broadcast. */
export function guideSelection(data: LiveTvState): { channel?: TvChannel; program?: TvProgram } {
  const channels = visibleGuideChannels(data);
  const pairs = channels.flatMap(channel => channelPrograms(channel, data.programs)
    .filter(p => programPosition(p, data.windowStart) && matchesTvCategory(p, data.category)).map(program => ({ channel, program })));
  const selected = pairs.find(p => p.program.id === data.selectedProgramId && p.channel.id === data.selectedChannelId);
  const selectedChannel = channels.find(c => c.id === data.selectedChannelId);
  const candidates = selectedChannel ? pairs.filter(p => p.channel.id === selectedChannel.id) : pairs;
  const now = Date.now() / 1000;
  return selected ?? candidates.find(p => p.program.starts_at <= now && p.program.ends_at > now)
    ?? candidates[0] ?? { channel:selectedChannel ?? channels[0] };
}
