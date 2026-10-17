import { getArtworkUrl, type MediaItemDetail, type MediaItemSummary, type MediaPlaybackTarget } from '../api';
import { escapeHtml, formatDuration } from './format';
import { renderItemCard, renderMetadataRefreshBadge, renderWatchProgress } from './homeView';
import { renderRating } from './ratingView';
import { state } from './state';
import { renderButtonContent, renderIcon } from './ui';

export function episodeCode(item: { season_number?: number | null; episode_number?: number | null }): string {
  const season = typeof item.season_number === 'number' ? `S${String(item.season_number).padStart(2, '0')}` : '';
  const episode = typeof item.episode_number === 'number' ? `E${String(item.episode_number).padStart(2, '0')}` : '';
  return season + episode;
}

export function showForItem(item: MediaItemDetail): MediaItemSummary | undefined {
  return item.item_type === 'show' ? item : item.hierarchy.find((ancestor) => ancestor.item_type === 'show');
}

/** Use the item's cover and inherit missing backdrops from the closest ancestor. */
export function itemArtwork(item: MediaItemDetail): { poster?: string; logo?: string; backdrop?: string } {
  const show = showForItem(item);
  const isSeason = item.item_type === 'season';
  const coverRevision = item.artwork_updated_at ?? (isSeason ? show?.artwork_updated_at : undefined);
  const poster = item.poster_url || isSeason ? getArtworkUrl(item.id, 'poster', coverRevision) : undefined;
  const logoItem = show?.logo_url ? show : item;
  const logo = logoItem.logo_url ? getArtworkUrl(logoItem.id, 'logo', logoItem.artwork_updated_at) : undefined;
  const metadataBackdrop = state.selectedItemMetadata?.item_id === item.id
    && state.selectedItemMetadata.matches.some((match) => match.backdrop_url || match.cached_backdrop_path);
  let backdropItem: MediaItemSummary | undefined = item.backdrop_url || metadataBackdrop ? item : undefined;
  backdropItem ??= [...item.hierarchy].reverse().find((ancestor) => ancestor.backdrop_url);
  return { poster, logo, backdrop: backdropItem ? getArtworkUrl(backdropItem.id, 'backdrop', backdropItem.artwork_updated_at) : undefined };
}

/** Keep the series identity visible alongside the season or episode's specific title. */
export function renderTvTitle(item: MediaItemDetail, logo?: string): string {
  const show = showForItem(item);
  const episodeTitle = [episodeCode(item), item.display_title].filter(Boolean).join(' · ');
  const title = show?.display_title ?? (item.item_type === 'episode' ? episodeTitle : item.display_title);
  const heading = logo
    ? `<img class="item-title-logo" src="${escapeHtml(logo)}" alt="${escapeHtml(title)}" />`
    : `<h2 class="item-title-fallback">${escapeHtml(title)}</h2>`;
  if (!show || !['season', 'episode'].includes(item.item_type)) return heading;
  const context = item.item_type === 'episode' ? episodeTitle : item.display_title;
  return `${heading}<h2 class="item-context-title">${escapeHtml(context)}</h2>`;
}

function episodeOrder(left: MediaItemSummary, right: MediaItemSummary): number {
  return (left.season_number ?? 0) - (right.season_number ?? 0)
    || (left.episode_number ?? 0) - (right.episode_number ?? 0) || left.id - right.id;
}

/** Siblings stay within the current season and library, including before the full catalog arrives. */
export function seasonEpisodes(item: MediaItemDetail): MediaItemSummary[] {
  const parentId = item.item_type === 'season' ? item.id : item.parent_id;
  if (parentId == null) return [];
  const candidates = [...state.libraryItems, ...item.children, ...(item.item_type === 'episode' ? [item] : [])];
  return [...new Map(candidates.filter((entry) => entry.item_type === 'episode'
    && entry.parent_id === parentId && entry.library_id === item.library_id).map((entry) => [entry.id, entry])).values()].sort(episodeOrder);
}

export function renderEpisodeNavigation(item: MediaItemDetail): string {
  if (item.item_type !== 'episode') return '';
  const episodes = seasonEpisodes(item);
  const index = episodes.findIndex((episode) => episode.id === item.id);
  return [{ episode: episodes[index - 1], label: 'Previous episode', icon: 'chevron-left' as const }, { episode: episodes[index + 1], label: 'Next episode', icon: 'chevron-right' as const }]
    .map(({ episode, label, icon }) => {
      if (!episode) return '';
      const title = `${label}: ${episodeCode(episode)} · ${episode.display_title}`;
      return `<button type="button" class="secondary-button episode-navigation-button" data-item-id="${episode.id}" title="${escapeHtml(title)}">${renderButtonContent(label, icon)}</button>`;
    }).join('');
}

/** Restart targets launch the first available episode; they do not erase watch history. */
export function tvRestartTargets(item: MediaItemDetail): MediaPlaybackTarget[] {
  const show = showForItem(item);
  const season = item.item_type === 'season' ? item : item.hierarchy.find((ancestor) => ancestor.item_type === 'season');
  const catalog = [...state.libraryItems, ...item.children];
  if (item.item_type === 'episode') catalog.push(item);
  const targets: MediaPlaybackTarget[] = [];
  for (const container of [season, show]) {
    if (!container) continue;
    const target = restartTargetForContainer(container, item, catalog, season);
    if (target) targets.push(target);
  }
  return targets;
}

function restartTargetForContainer(container: MediaItemSummary, item: MediaItemDetail, catalog: MediaItemSummary[], season?: MediaItemSummary): MediaPlaybackTarget | undefined {
  const supplied = container.id === item.id ? item.restart_playback_target : undefined;
  // A partial catalog cannot identify an ancestor's first episode reliably.
  if (!supplied && (state.libraryItemsLoading || !state.libraryItems.some((entry) => entry.id === container.id))) return undefined;
  const first = firstRestartEpisode(container, item.library_id, catalog, season);
  const target = supplied ?? (first ? { item_id: first.id, start_ms: 0, display_title: first.display_title, season_number: first.season_number, episode_number: first.episode_number, resume: false, label: '' } : undefined);
  if (!target) return undefined;
  return { ...target, start_ms: 0, label: container.item_type === 'show' ? 'Restart show' : 'Restart season' };
}

/** Find an available episode within the requested season or series. */
function firstRestartEpisode(container: MediaItemSummary, libraryId: number, catalog: MediaItemSummary[], season?: MediaItemSummary): MediaItemSummary | undefined {
  const seasonIds = new Set(catalog.filter((entry) => entry.item_type === 'season' && entry.parent_id === container.id).map((entry) => entry.id));
  if (season && container.item_type === 'show') seasonIds.add(season.id);
  return catalog.filter((entry) => entry.item_type === 'episode' && entry.playable && !entry.missing_since
    && entry.library_id === libraryId && (container.item_type === 'season' ? entry.parent_id === container.id : seasonIds.has(entry.parent_id!))).sort(episodeOrder)[0];
}

export function tvPlaybackLabel(item: MediaItemDetail, target: MediaPlaybackTarget): string {
  if (item.item_type !== 'show' && item.item_type !== 'season') return target.label;
  const started = target.resume || target.start_ms > 0 || (item.playback_position_ms ?? 0) > 0 || (item.watch_count ?? 0) > 0;
  const code = episodeCode(target);
  return `${started ? 'Continue' : 'Play'} ${code || target.display_title}`;
}

/** Season tabs share the sticky action bar, so they remain available while browsing episodes. */
export function renderSeasonTabs(item: MediaItemDetail): string {
  if (item.item_type !== 'season') return '';
  const seasons = state.libraryItems.filter((entry) => entry.item_type === 'season' && entry.parent_id === item.parent_id && entry.library_id === item.library_id);
  if (!seasons.some((season) => season.id === item.id)) seasons.push(item);
  seasons.sort((left, right) => (left.season_number ?? 0) - (right.season_number ?? 0) || left.id - right.id);
  const tabs = seasons.map((season) => `<button type="button" class="season-tab secondary-button ${season.id === item.id ? 'active' : ''}" data-item-id="${season.id}" ${season.id === item.id ? 'aria-current="page"' : ''}>${escapeHtml(season.display_title)}</button>`).join('');
  return `<nav class="season-tabs" aria-label="Seasons" data-nav-section="season-tabs" data-nav-label="Seasons">${tabs}</nav>`;
}

export function renderTvEpisodeCard(episode: MediaItemSummary): string {
  const artwork = getArtworkUrl(episode.id, 'poster', episode.artwork_updated_at);
  return `<button type="button" class="media-extra-card episode-card" data-item-id="${episode.id}">
    <span class="media-extra-thumbnail episode-thumbnail has-image"><img data-episode-still src="${escapeHtml(artwork)}" alt="" loading="lazy" /><span class="item-cover-fallback episode-still-fallback" hidden aria-hidden="true">${renderIcon('tv', 'media-extra-placeholder-icon')}</span>
      <span class="episode-number-badge">${escapeHtml(episodeCode(episode))}</span>${renderWatchProgress(episode)}
      ${renderMetadataRefreshBadge(episode)}
    </span>
    <strong class="media-extra-title">${escapeHtml(episode.display_title)}</strong>
    <span class="media-extra-meta"><span>${escapeHtml(formatDuration(episode.duration_ms))}</span>${renderRating(episode)}</span>
    ${episode.overview ? `<span class="episode-card-overview">${escapeHtml(episode.overview)}</span>` : ''}
  </button>`;
}

function renderTvShelf(title: string, items: MediaItemSummary[], kind: 'seasons' | 'episodes', id: string, section: string): string {
  const cards = kind === 'seasons' ? items.map(renderItemCard).join('') : items.map(renderTvEpisodeCard).join('');
  return `<section class="item-section tv-item-section" data-nav-section="${section}" data-nav-label="${escapeHtml(title)}">
    <div class="section-heading item-row-heading"><h3>${escapeHtml(title)} <span class="item-row-count">· ${items.length}</span></h3></div>
    <div class="shelf-row-shell item-row-shell">
      <button type="button" class="shelf-scroll-button" data-shelf-scroll="${id}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
      <div class="item-responsive-row tv-item-row" data-item-row="${kind}" data-shelf-row="${id}">${cards}</div>
      <button type="button" class="shelf-scroll-button" data-shelf-scroll="${id}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
    </div>
  </section>`;
}

export function renderTvChildren(item: MediaItemDetail): string {
  if (item.item_type === 'show') {
    const seasons = item.children.filter((child) => child.item_type === 'season').sort((left, right) => (left.season_number ?? 0) - (right.season_number ?? 0) || left.id - right.id);
    return seasons.length ? renderTvShelf('Seasons', seasons, 'seasons', 'show-seasons', 'children') : '';
  }
  if (item.item_type === 'season') {
    const episodes = seasonEpisodes(item);
    return episodes.length ? renderTvShelf('Episodes', episodes, 'episodes', 'season-episodes', 'children') : '<section class="item-section"><div class="section-heading"><h3>Episodes <span class="item-row-count">· 0</span></h3></div><div class="empty-state">No episodes are available in this season.</div></section>';
  }
  if (item.item_type === 'episode') {
    const siblings = seasonEpisodes(item).filter((episode) => episode.id !== item.id);
    return siblings.length ? renderTvShelf('More from this season', siblings, 'episodes', 'sibling-episodes', 'siblings') : '';
  }
  return '';
}
