/** Renders home, browse, shelf, and media-card markup. */
import type { MediaCollectionSummary, MediaItemSummary, MediaLibrary, MediaPlaybackTarget, MediaSearchResult, MediaShelf } from '../api';
import { getArtworkUrl, getPersonImageUrl, resolveApiUrl } from '../api';
import { HOME_SHELF_CHUNK_SIZE } from './constants';
import { escapeHtml, formatTimestamp } from './format';
import { activeBrowseFilterCount, browseItems } from './browsePreferences';
import { renderLoadingCards, renderLoadingDetail } from './loadingView';
import { currentThemeSongYouTubeTarget } from './mediaTargets';
import { playbackProgressPercent } from './playbackProgress';
import { state } from './state';
import { canManageServer } from './auth';
import type { BrowseFilter, HomeBrowseTab } from './types';
import {
  activeLibrary,
  activeLibraryId,
  categoryForRoute,
  categorySummaries,
  collectionForRoute,
  collectionSummaries,
  filteredTopLevelLibraryItems,
  homeFeaturePreview,
  itemsForCollection,
  pageBackdropUrlForCollection,
  pageBackdropUrlForItem,
  topLevelLibraryItems,
} from './selectors';
import {
  activeLibraryPendingRefreshCount,
  hasActiveLibraryScan,
  itemIsMetadataPending,
  libraryHasActiveMetadataRefresh,
  metadataRefreshActivityProgressForLibrary,
} from './activities';
import {
  formatChildCount,
  humanizeItemType,
  libraryStatusLabel,
  renderButtonContent,
  renderCollapsibleText,
  renderIcon,
  selectedLibraryIcon,
} from './ui';

export const LIBRARY_PAGE_SIZE = 60;

export function browseDetailPath(kind: BrowseFilter['kind'], key: string): string {
  let segment = 'categories';
  if (kind === 'collection') {
    segment = 'collections';
  } else if (kind === 'playlist') {
    segment = 'playlists';
  }
  const encodedKey = encodeURIComponent(key);
  return typeof activeLibraryId() === 'number'
    ? `/libraries/${activeLibraryId()}/items/${segment}/${encodedKey}`
    : `/items/${segment}/${encodedKey}`;
}

function browseFilterKindLabel(kind: BrowseFilter['kind']): string {
  if (kind === 'collection') {
    return 'Collection';
  }
  if (kind === 'playlist') {
    return 'Playlist';
  }
  return 'Category';
}

export function homeBrowsePath(): string {
  const libraryId = activeLibraryId();
  return typeof libraryId === 'number' ? `/libraries/${libraryId}` : '/';
}

export function browseFilterForRoute(): BrowseFilter | undefined {
  if (state.route.page !== 'browse-detail') {
    return undefined;
  }
  const route = state.route;

  if (route.kind === 'collection') {
    const collection = collectionSummaries().find((entry) => entry.id === route.key);
    if (!collection) {
      return undefined;
    }

    return {
      kind: 'collection',
      label: collection.name,
      itemIds: collection.item_ids,
      overview: collection.overview,
      artworkUrl: collection.backdrop_url ?? collection.artwork_url,
    };
  }

  if (route.kind === 'playlist') {
    return {
      kind: 'playlist',
      label: route.key,
      itemIds: [],
      overview: 'No playlist items are available yet.',
    };
  }

  const category = categorySummaries().find((entry) => entry.genre === route.key);
  if (!category) {
    return undefined;
  }

  return {
    kind: 'category',
    label: category.genre,
    itemIds: category.items.map((item) => item.id),
    overview: category.items.slice(0, 5).map((item) => item.display_title).join(' · '),
  };
}

export function renderBrowseFilterDetail(): string {
  const filter = state.route.page === 'browse-detail' ? browseFilterForRoute() : state.browseFilter;
  if (!filter) {
    if (state.libraryItemsLoading) {
      return renderLoadingCards('Loading library items');
    }
    return '<div class="empty-state">This page is no longer available for the current library.</div>';
  }

  const allowedIds = new Set(filter.itemIds);
  const items = topLevelLibraryItems().filter((item) => allowedIds.has(item.id));
  const artworkStyle = filter.artworkUrl
    ? `style="--home-feature-image: url('${escapeHtml(filter.artworkUrl)}');"`
    : '';
  const themeSongOption = currentThemeSongYouTubeTarget();
  const filterKindLabel = browseFilterKindLabel(filter.kind);
  const filterOverview = filter.overview ?? `${items.length} title${items.length === 1 ? '' : 's'} in this ${filter.kind}.`;

  return `
    <section class="browse-filter-detail">
      <div class="home-feature ${filter.artworkUrl ? 'has-artwork' : ''}" data-nav-section="featured" data-nav-label="Overview" ${artworkStyle}>
        <div class="home-feature-copy">
          <p class="eyebrow">${escapeHtml(filterKindLabel)}</p>
          <h2>${escapeHtml(filter.label)}</h2>
          <p>${escapeHtml(filterOverview)}</p>
          <div class="hero-meta-row">
            <span class="tag">${items.length} title${items.length === 1 ? '' : 's'}</span>
          </div>
        </div>
        <div class="home-feature-actions">
          ${themeSongOption ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>` : ''}
          <button type="button" class="secondary-button" id="clear-browse-filter">
            ${renderButtonContent('Back', 'arrow-left')}
          </button>
        </div>
      </div>
      <div class="item-grid" data-nav-section="items" data-nav-label="Items">${items.map(renderItemCard).join('')}</div>
    </section>
  `;
}

export function renderCollectionDetailPage(): string {
  const collection = collectionForRoute();
  if (!collection) {
    if (state.libraryItemsLoading) {
      return renderLoadingDetail('Loading collection');
    }
    return '<div class="empty-state">This collection is no longer available for the current library.</div>';
  }

  const items = itemsForCollection(collection);
  const themeSongOption = currentThemeSongYouTubeTarget();
  const posterUrl = collection.artwork_url ? resolveApiUrl(collection.artwork_url) : undefined;
  const overview = collection.overview ?? 'No description is stored for this collection yet.';

  return `
    <section class="item-page collection-page">
      <section class="item-hero collection-hero" data-nav-section="overview" data-nav-label="Overview">
        <div class="detail-art item-poster collection-poster ${posterUrl ? 'has-image' : ''}">
          ${posterUrl ? `<img src="${escapeHtml(posterUrl)}" alt="${escapeHtml(collection.name)} poster" />` : renderIcon('image', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Collection</p>
          <h2 class="item-title-fallback">${escapeHtml(collection.name)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${items.length} title${items.length === 1 ? '' : 's'}</span>
          </div>
          ${renderCollapsibleText(overview, `collection-overview:${collection.id}`)}
          <div class="detail-actions">
            ${themeSongOption ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>` : ''}
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section" data-nav-section="items" data-nav-label="Items">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">${items.length} item${items.length === 1 ? '' : 's'}</span>
        </div>
        ${items.length
          ? `<div class="item-grid hierarchy-item-grid">${items.map(renderItemCard).join('')}</div>`
          : '<div class="empty-state tight">No titles are currently linked to this collection.</div>'}
      </section>
    </section>
  `;
}

export function renderCategoryDetailPage(): string {
  const category = categoryForRoute();
  if (!category) {
    if (state.libraryItemsLoading) {
      return renderLoadingDetail('Loading genre');
    }
    return '<div class="empty-state">This genre is no longer available for the current library.</div>';
  }

  const overview = category.items.slice(0, 5).map((item) => item.display_title).join(' · ')
    || 'No titles are currently linked to this genre.';

  return `
    <section class="item-page grouped-page category-detail-page">
      <section class="item-hero collection-hero" data-nav-section="overview" data-nav-label="Overview">
        <div class="detail-art item-poster collection-poster">
          ${renderIcon('layout-grid', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Genre</p>
          <h2 class="item-title-fallback">${escapeHtml(category.genre)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${category.items.length} title${category.items.length === 1 ? '' : 's'}</span>
          </div>
          ${renderCollapsibleText(overview, `category-overview:${category.genre}`)}
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section" data-nav-section="items" data-nav-label="Items">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">${category.items.length} item${category.items.length === 1 ? '' : 's'}</span>
        </div>
        ${category.items.length
          ? `<div class="item-grid hierarchy-item-grid">${category.items.map(renderItemCard).join('')}</div>`
          : '<div class="empty-state tight">No titles are currently linked to this genre.</div>'}
      </section>
    </section>
  `;
}

export function renderPlaylistDetailPage(): string {
  const route = state.route;
  const playlistName = route.page === 'browse-detail' && route.kind === 'playlist'
    ? route.key
    : 'Playlist';

  return `
    <section class="item-page grouped-page playlist-detail-page">
      <section class="item-hero collection-hero" data-nav-section="overview" data-nav-label="Overview">
        <div class="detail-art item-poster collection-poster">
          ${renderIcon('play', 'audio-player-art-icon')}
        </div>
        <div class="detail-summary item-summary">
          <p class="eyebrow">Playlist</p>
          <h2 class="item-title-fallback">${escapeHtml(playlistName)}</h2>
          <div class="hero-meta-row">
            <span class="tag">0 titles</span>
          </div>
          <p>No playlist items are available yet.</p>
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Back', 'arrow-left')}</button>
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section" data-nav-section="items" data-nav-label="Items">
        <div class="section-heading section-heading-actions">
          <h3>Items</h3>
          <span class="muted">0 items</span>
        </div>
        <div class="empty-state tight">Playlist creation is planned. Items will appear here when playlists are available.</div>
      </section>
    </section>
  `;
}

export function renderBrowseDetailPage(): string {
  if (state.route.page !== 'browse-detail') {
    return renderBrowseFilterDetail();
  }

  switch (state.route.kind) {
    case 'collection':
      return renderCollectionDetailPage();
    case 'category':
      return renderCategoryDetailPage();
    case 'playlist':
      return renderPlaylistDetailPage();
  }

  return renderBrowseFilterDetail();
}

export function metadataBadgeMarkup(item: MediaItemSummary): string {
  const pending = itemIsMetadataPending(item);
  const unmatched = !item.has_metadata;
  if (!pending && !unmatched) {
    return '';
  }

  let statusLabel = 'Metadata is not linked yet';
  if (pending) {
    statusLabel = unmatched ? 'Matching metadata' : 'Refreshing metadata';
  }
  return `
    <span class="media-card-status ${unmatched ? 'is-unmatched' : ''} ${pending ? 'is-loading' : ''} ${pending && unmatched ? 'has-multiple' : 'icon-only'}" title="${escapeHtml(statusLabel)}" aria-label="${escapeHtml(statusLabel)}">
      ${unmatched ? `<span class="status-warning-icon">${renderIcon('triangle-alert', 'status-icon')}</span>` : ''}
      ${pending ? '<span class="loading-spinner" aria-hidden="true"></span>' : ''}
    </span>
  `;
}

export function missingItemBadgeMarkup(item: MediaItemSummary): string {
  if (!item.missing_since) {
    return '';
  }

  const statusLabel = `Missing from disk since ${formatTimestamp(item.missing_since)}`;
  return `
    <span class="media-card-status is-missing" title="${escapeHtml(statusLabel)}" aria-label="${escapeHtml(statusLabel)}">
      ${renderIcon('triangle-alert', 'status-icon')}
      <span>Missing</span>
    </span>
  `;
}

export function missingItemDetailBadgeMarkup(item: MediaItemSummary): string {
  if (!item.missing_since) {
    return '';
  }

  const statusLabel = `Missing from disk since ${formatTimestamp(item.missing_since)}`;
  return `
    <span class="tag warning status-tag" title="${escapeHtml(statusLabel)}" aria-label="${escapeHtml(statusLabel)}">
      ${renderIcon('triangle-alert', 'status-icon')}
      <span>Missing</span>
    </span>
  `;
}

export function playbackStatusBadgeMarkup(item: MediaItemSummary): string {
  return renderWatchProgress(item);
}

/** One artwork indicator: current playback progress, otherwise a completed watch. */
export function renderWatchProgress(item: MediaItemSummary): string {
  const progress = playbackProgressPercent(item);
  if (progress !== undefined) {
    const label = `In progress: ${progress}% watched`;
    return `<span class="media-card-progress" style="--watch-progress: ${progress}%" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}"></span>`;
  }
  const watchCount = item.watch_count ?? 0;
  if (!item.playback_completed && watchCount <= 0) return '';
  const label = watchCount > 1 ? `Watched ${watchCount} times` : 'Watched';
  const title = item.last_watched_at ? `${label} · Last watched ${formatTimestamp(item.last_watched_at)}` : label;
  return `<span class="media-card-progress is-watched" style="--watch-progress: 100%" title="${escapeHtml(title)}" aria-label="${escapeHtml(label)}">${renderIcon('check', 'watch-completed-icon')}</span>`;
}

interface PlaybackTargetButtonOptions {
  holdForOptions?: boolean;
  subtitle?: string;
  id?: string;
}

export function renderPlaybackTargetButton(target: MediaPlaybackTarget, secondary: boolean, { holdForOptions, subtitle, id }: PlaybackTargetButtonOptions = {}): string {
  return `
    <button type="button" ${id ? `id="${escapeHtml(id)}"` : ''} class="${secondary ? 'secondary-button' : ''}" ${holdForOptions ? `data-resume-hold aria-describedby="resume-hold-hint" aria-controls="resume-playback-menu" aria-expanded="${state.resumeOptionsOpen}"` : ''} data-playback-target-item-id="${target.item_id}" data-playback-target-start-ms="${target.start_ms}" title="${escapeHtml(target.display_title)}">
      ${renderButtonContent(target.label, secondary ? 'rotate-ccw' : 'play', 'start', subtitle)}
    </button>
  `;
}

export function itemCardSubtitle(item: MediaItemSummary): string | undefined {
  if (item.display_subtitle) {
    return item.display_subtitle === item.display_title ? undefined : item.display_subtitle;
  }

  if (item.item_type === 'episode' && typeof item.episode_number === 'number') {
    return `Episode ${item.episode_number}`;
  }

  if (item.item_type === 'season' && typeof item.season_number === 'number') {
    const subtitle = `Season ${item.season_number}`;
    return subtitle === item.display_title ? undefined : subtitle;
  }

  return undefined;
}

function mediaCardBadgeGroup(markup: string, className: string): string {
  if (!markup) {
    return '';
  }

  return `<span class="${className}">${markup}</span>`;
}

function mediaCardDynamicBadges(badgeMarkup: string, playbackBadgeMarkup: string): string {
  const badgeGroups = [
    mediaCardBadgeGroup(badgeMarkup, 'media-card-state-badges'),
    mediaCardBadgeGroup(playbackBadgeMarkup, 'media-card-playback-badges'),
  ].join('');
  if (!badgeGroups) {
    return '';
  }

  return `
      <span class="media-card-dynamic-badges">
        ${badgeGroups}
      </span>
    `;
}

export function renderItemCard(item: MediaItemSummary): string {
  const library = state.libraries.find((entry) => entry.id === item.library_id);
  const artworkItemId = item.artwork_item_id ?? item.id;
  const artworkUrl = getArtworkUrl(artworkItemId, 'poster', item.artwork_updated_at);
  const hasAlternateArtwork = typeof item.artwork_item_id === 'number' && item.artwork_item_id !== item.id;
  const useEpisodeLayout = item.item_type === 'episode' && !hasAlternateArtwork;
  const artworkTypeClass = useEpisodeLayout ? item.item_type : 'poster-art';
  const cardSubtitle = itemCardSubtitle(item);
  const isSeasonEpisodeCard = state.route.page === 'item'
    && state.selectedItem?.item_type === 'season'
    && item.item_type === 'episode';
  const showLibrary = activeLibraryId() === undefined;
  const secondaryMeta = isSeasonEpisodeCard ? '' : [showLibrary ? library?.name : '', item.release_year].filter(Boolean).join(' · ');
  const metricMarkup = item.missing_since
    ? missingItemBadgeMarkup(item)
    : `<span class="media-card-duration">${escapeHtml(formatChildCount(item))}</span>`;
  const dynamicBadges = mediaCardDynamicBadges(metadataBadgeMarkup(item), playbackStatusBadgeMarkup(item));

  return `
    <button class="media-card ${useEpisodeLayout ? 'episode-card' : ''} ${item.missing_since ? 'is-missing' : ''}" type="button" data-item-id="${item.id}" data-preview-item-id="${item.id}">
      <span class="media-card-art ${escapeHtml(item.media_kind)} ${escapeHtml(artworkTypeClass)}" style="background-image: url('${escapeHtml(artworkUrl)}');">
        <span class="media-card-kind-row">
          ${showLibrary ? `<span class="media-card-kind" title="${escapeHtml(humanizeItemType(item.item_type))}">${renderIcon(selectedLibraryIcon(library?.kind), 'card-icon')}</span>` : ''}
          ${metricMarkup}
        </span>
        ${dynamicBadges}
      </span>
      <span class="media-card-title">${escapeHtml(item.display_title)}</span>
      ${cardSubtitle ? `<span class="media-card-subtitle">${escapeHtml(cardSubtitle)}</span>` : ''}
      ${secondaryMeta ? `<span class="media-card-meta">${escapeHtml(secondaryMeta)}</span>` : ''}
    </button>
  `;
}

export function renderHomeFeature(): string {
  const preview = homeFeaturePreview();
  if (!preview) {
    if (state.isLoading) return '<section class="home-feature home-feature-loading" aria-label="Loading preview" aria-busy="true"><div class="home-feature-copy" aria-hidden="true"><div class="loading-line skeleton"></div><div class="loading-line short skeleton"></div></div></section>';
    return `<section class="home-feature home-feature-empty"><div class="home-feature-copy"><h2>${state.homeTab === 'library' ? 'No matching titles' : 'No preview available'}</h2><p>${state.homeTab === 'library' ? 'Change a filter below to find something to watch.' : 'Details will appear here when media is available.'}</p></div></section>`;
  }

  if (preview.kind === 'collection') {
    return renderCollectionFeature(preview.collection);
  }
  return renderItemFeature(preview.item);
}

function featureArtworkStyle(backdropUrl?: string): string {
  return backdropUrl ? `style="--home-feature-image: url('${escapeHtml(backdropUrl)}');"` : '';
}

function renderCollectionFeature(collection: MediaCollectionSummary): string {
  const backdropUrl = pageBackdropUrlForCollection(collection);
  const overview = collection.overview ?? `${collection.item_count} title${collection.item_count === 1 ? '' : 's'} in this collection.`;
  return `
      <section class="home-feature${backdropUrl ? ' has-artwork' : ''}" data-nav-section="featured" data-nav-label="Featured" ${featureArtworkStyle(backdropUrl)}>
        <div class="home-feature-copy">
          <p class="eyebrow">Collection</p>
          <h2>${escapeHtml(collection.name)}</h2>
          <p>${escapeHtml(overview)}</p>
          <div class="hero-meta-row">
            <span class="tag">${collection.item_count} title${collection.item_count === 1 ? '' : 's'}</span>
          </div>
        </div>
        <button type="button" class="secondary-button home-feature-action" data-collection-filter="${escapeHtml(collection.id)}">
          ${renderButtonContent('Open', 'arrow-right')}
        </button>
      </section>
    `;
}

function renderItemFeature(item: MediaItemSummary): string {
  const backdropUrl = pageBackdropUrlForItem(item);
  const logoUrl = item.logo_url ? getArtworkUrl(item.id, 'logo', item.artwork_updated_at) : undefined;
  const library = state.libraries.find((entry) => entry.id === item.library_id);
  const genreMarkup = item.genres.slice(0, 3).map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`).join('');
  const overview = item.overview ?? `${humanizeItemType(item.item_type)} from ${library?.name ?? 'your library'}.`;

  return `
    <section class="home-feature${backdropUrl ? ' has-artwork' : ''}" data-nav-section="featured" data-nav-label="Featured" ${featureArtworkStyle(backdropUrl)}>
      <div class="home-feature-copy">
        ${logoUrl
          ? `<img class="home-feature-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(item.display_title)}" />`
          : `<h2>${escapeHtml(item.display_title)}</h2>`}
        <p>${escapeHtml(overview)}</p>
        <div class="hero-meta-row">
          ${genreMarkup}
          <span class="tag">${escapeHtml(formatChildCount(item))}</span>
        </div>
      </div>
      <button type="button" class="secondary-button home-feature-action" data-item-id="${item.id}">
        ${renderButtonContent('Open', 'arrow-right')}
      </button>
    </section>
  `;
}

type ItemSearchResult = Extract<MediaSearchResult, { result_type: 'item' }>;
type CollectionSearchResult = Extract<MediaSearchResult, { result_type: 'collection' }>;
type PersonSearchResult = Extract<MediaSearchResult, { result_type: 'person' }>;
type PlaylistSearchResult = Extract<MediaSearchResult, { result_type: 'playlist' }>;

function renderItemSearchResultRow(result: ItemSearchResult, compact: boolean): string {
  const item = result.item;
  const posterUrl = getArtworkUrl(item.id, 'poster', item.artwork_updated_at);
  const library = state.libraries.find((entry) => entry.id === item.library_id);
  const itemResultDetails = [library?.name ?? 'Library', humanizeItemType(item.item_type)];
  if (!compact) {
    itemResultDetails.push(formatChildCount(item));
  }

  return `
    <button type="button" class="search-result-row" data-item-id="${item.id}" data-preview-item-id="${item.id}">
      <span class="search-result-thumb" style="background-image: url('${escapeHtml(posterUrl)}');"></span>
      <span class="search-result-copy">
        <strong>${escapeHtml(item.display_title)}</strong>
        <span>${escapeHtml(itemResultDetails.join(' · '))}</span>
        ${!compact && item.overview ? `<small>${escapeHtml(item.overview)}</small>` : ''}
      </span>
    </button>
  `;
}

function renderCollectionSearchResultRow(result: CollectionSearchResult, compact: boolean): string {
  const collection = result.collection;
  const posterUrl = collection.artwork_url ?? collection.backdrop_url;
  return `
    <button type="button" class="search-result-row" data-collection-filter="${escapeHtml(collection.id)}" data-preview-collection-id="${escapeHtml(collection.id)}">
      <span class="search-result-thumb" ${posterUrl ? `style="background-image: url('${escapeHtml(resolveApiUrl(posterUrl))}');"` : ''}>${posterUrl ? '' : renderIcon('image')}</span>
      <span class="search-result-copy">
        <strong>${escapeHtml(collection.name)}</strong>
        <span>${escapeHtml(`Collection · ${collection.item_count} title${collection.item_count === 1 ? '' : 's'}`)}</span>
        ${!compact && collection.overview ? `<small>${escapeHtml(collection.overview)}</small>` : ''}
      </span>
    </button>
  `;
}

function renderPersonSearchResultRow(result: PersonSearchResult, compact: boolean): string {
  const person = result.person;
  const imageUrl = person.cached_image_path || person.image_url ? getPersonImageUrl(person.id) : undefined;
  const knownFor = person.known_for.slice(0, 3).join(' · ');
  return `
    <button type="button" class="search-result-row" data-person-id="${person.id}">
      <span class="search-result-thumb" ${imageUrl ? `style="background-image: url('${escapeHtml(imageUrl)}');"` : ''}>${imageUrl ? '' : renderIcon('user-plus')}</span>
      <span class="search-result-copy">
        <strong>${escapeHtml(person.name)}</strong>
        <span>${escapeHtml(knownFor ? `Person · ${knownFor}` : 'Person')}</span>
        ${!compact && person.biography ? `<small>${escapeHtml(person.biography)}</small>` : ''}
      </span>
    </button>
  `;
}

function renderPlaylistSearchResultRow(result: PlaylistSearchResult, compact: boolean): string {
  const playlist = result.playlist;
  return `
    <button type="button" class="search-result-row" data-playlist-filter="${escapeHtml(playlist.id)}">
      <span class="search-result-thumb">${renderIcon('music')}</span>
      <span class="search-result-copy">
        <strong>${escapeHtml(playlist.name)}</strong>
        <span>${escapeHtml(`Playlist · ${playlist.item_count} title${playlist.item_count === 1 ? '' : 's'}`)}</span>
        ${!compact && playlist.overview ? `<small>${escapeHtml(playlist.overview)}</small>` : ''}
      </span>
    </button>
  `;
}

export function renderSearchResultRow(result: MediaSearchResult, compact: boolean): string {
  switch (result.result_type) {
    case 'item':
      return renderItemSearchResultRow(result, compact);
    case 'collection':
      return renderCollectionSearchResultRow(result, compact);
    case 'person':
      return renderPersonSearchResultRow(result, compact);
    case 'playlist':
      return renderPlaylistSearchResultRow(result, compact);
  }
}

export function renderSearchResults(): string {
  if (state.searchLoading) return renderLoadingCards('Searching your library');
  if (!state.searchResults.length) {
    return '<section class="shelf"><div class="empty-state">No library content matched the current search.</div></section>';
  }

  return `
    <section class="search-results-section" data-nav-section="search-results" data-nav-label="Search results">
      <div class="shelf-header">
        <h3>Search results</h3>
        <span>${state.searchResults.length} matches</span>
      </div>
      <div class="search-results-list">
        ${state.searchResults.map((result) => renderSearchResultRow(result, false)).join('')}
      </div>
    </section>
  `;
}

export function visibleShelfItems(shelf: MediaShelf): MediaItemSummary[] {
  return shelf.items.slice(0, HOME_SHELF_CHUNK_SIZE);
}

export function renderShelfStack(): string {
  const shelves = (state.home?.shelves ?? []).filter((shelf) => shelf.items.length);
  if (!shelves.length) {
    if (state.isLoading) return renderLoadingCards('Loading your media');
    return '<section class="shelf"><div class="empty-state">No shelves are available yet. Add a library to get started.</div></section>';
  }

  return shelves
    .map((shelf) => `
      <section class="shelf" data-nav-section="shelf:${escapeHtml(shelf.id)}" data-nav-label="${escapeHtml(shelf.title)}" aria-label="${escapeHtml(shelf.title)}">
        <div class="shelf-header">
          <h3>${escapeHtml(shelf.title)}</h3>
          <span>${shelf.items.length} items</span>
        </div>
        <div class="shelf-row-shell">
              <button type="button" class="shelf-scroll-button" data-shelf-scroll="${escapeHtml(shelf.id)}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
              <div
                class="shelf-row"
                data-shelf-row="${escapeHtml(shelf.id)}"
                data-lazy-shelf-id="${escapeHtml(shelf.id)}"
                data-lazy-rendered-count="${Math.min(HOME_SHELF_CHUNK_SIZE, shelf.items.length)}"
              >${visibleShelfItems(shelf).map(renderItemCard).join('')}</div>
              <button type="button" class="shelf-scroll-button" data-shelf-scroll="${escapeHtml(shelf.id)}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
            </div>
      </section>
    `)
    .join('');
}

export function renderHomeTabs(): string {
  const tabs: Array<{ id: HomeBrowseTab; label: string }> = [
    { id: 'recommended', label: 'Recommended' },
    { id: 'library', label: 'Library' },
    { id: 'collections', label: 'Collections' },
    { id: 'playlists', label: 'Playlists' },
    { id: 'categories', label: 'Categories' },
  ];

  return `
    <nav class="browse-tabs" aria-label="Browse views">
      ${tabs.map((tab) => `
        <button
          type="button"
          class="browse-tab-button ${state.homeTab === tab.id ? 'active' : ''}"
          data-home-tab="${tab.id}"
        >
          ${escapeHtml(tab.label)}
        </button>
      `).join('')}
    </nav>
  `;
}

type MetadataRefreshProgress = NonNullable<ReturnType<typeof metadataRefreshActivityProgressForLibrary>>;

function renderEmptyLibraryOverview(): string {
  return `
    <section class="panel page-panel library-overview-panel" data-nav-section="library-status" data-nav-label="Library status">
      <div class="library-overview-grid">
        <article class="library-stat-card">
          <span class="label">Libraries</span>
          <strong>${state.libraries.length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Items</span>
          <strong>${topLevelLibraryItems().length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Status</span>
          <strong>${state.libraries.some((entry) => entry.status === 'never_scanned') ? 'Pending scans' : 'Ready'}</strong>
        </article>
      </div>
    </section>
  `;
}

function renderLibraryRefreshStatusTag(library: MediaLibrary, activeRefreshProgress: MetadataRefreshProgress | undefined, stalePending: number): string {
  if (activeRefreshProgress) {
    return `<span class="tag warning">Refreshing metadata ${activeRefreshProgress.completed}/${activeRefreshProgress.total}</span>`;
  }

  return stalePending > 0
    ? `<span class="tag warning">Pending metadata ${library.metadata_refresh_completed}/${library.metadata_refresh_total}</span>`
    : '';
}

function libraryStatusTagClass(status: string): string {
  if (status === 'available') {
    return 'success';
  }
  if (status === 'never_scanned') {
    return 'warning';
  }
  return '';
}

function renderMetadataRefreshNote(activeRefreshProgress: MetadataRefreshProgress | undefined): string {
  const metadataRefreshFailedSuffix = activeRefreshProgress?.failed
    ? ` (${activeRefreshProgress.failed} failed)`
    : '';
  return activeRefreshProgress
    ? `<p class="muted library-overview-note">Metadata refresh progress: ${activeRefreshProgress.completed}/${activeRefreshProgress.total}${metadataRefreshFailedSuffix}. Artwork and item cards update automatically as each item completes.</p>`
    : '';
}

function renderStalePendingNote(stalePending: number): string {
  const stalePendingVerb = stalePending === 1 ? ' is' : 's are';
  return stalePending > 0
    ? `<p class="muted library-overview-note">${stalePending} item${stalePendingVerb} still marked pending without an active refresh worker. Use refresh metadata to resume the library refresh.</p>`
    : '';
}

export function renderLibraryOverview(): string {
  const library = activeLibrary();
  if (!library) {
    return renderEmptyLibraryOverview();
  }

  const activeRefreshProgress = metadataRefreshActivityProgressForLibrary(library.id);
  const stalePending = Math.max(0, library.metadata_refresh_pending - activeLibraryPendingRefreshCount(library.id));
  const scanPending = hasActiveLibraryScan(library.id);
  const refreshStatusTag = renderLibraryRefreshStatusTag(library, activeRefreshProgress, stalePending);
  const metadataRefreshNote = renderMetadataRefreshNote(activeRefreshProgress);
  const stalePendingNote = renderStalePendingNote(stalePending);

  return `
    <section class="panel page-panel library-overview-panel" data-nav-section="library-status" data-nav-label="Library status">
      <div class="library-overview-header">
        <div>
          <p class="eyebrow">Library overview</p>
          <h3>${escapeHtml(library.name)}</h3>
        </div>
        <div class="library-overview-actions">
          ${scanPending ? '<span class="tag warning">Scanning catalog</span>' : ''}
          ${refreshStatusTag}
          <div class="library-status-tags">
          <span class="tag ${libraryStatusTagClass(library.status)}">${escapeHtml(libraryStatusLabel(library.status))}</span>
          <span class="tag">${library.total_files} file${library.total_files === 1 ? '' : 's'}</span>
          </div>
        </div>
      </div>
      <div class="library-overview-grid">
        <article class="library-stat-card">
          <span class="label">Top-level items</span>
          <strong>${topLevelLibraryItems().length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Video files</span>
          <strong>${library.video_files}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Folders</span>
          <strong>${library.paths.length}</strong>
        </article>
        <article class="library-stat-card">
          <span class="label">Last scanned</span>
          <strong>${escapeHtml(formatTimestamp(library.last_scanned_at))}</strong>
        </article>
      </div>
      ${library.error ? `<p class="muted library-overview-note">${escapeHtml(library.error)}</p>` : ''}
      ${library.status === 'never_scanned' ? '<p class="muted library-overview-note">This library has not been scanned yet. It will populate after the next catalog scan starts.</p>' : ''}
      ${metadataRefreshNote}
      ${stalePendingNote}
    </section>
  `;
}

function browseSelect(name: string, label: string, value: string, options: Array<[string, string]>): string {
  const optionMarkup = options.map(([key, text]) => `<option value="${escapeHtml(key)}" ${key === value ? 'selected' : ''}>${escapeHtml(text)}</option>`).join('');
  return `<label>${label}<select name="${name}">${optionMarkup}</select></label>`;
}

export function renderLibraryToolbar(): string {
  const allItems = filteredTopLevelLibraryItems();
  const preferences = state.browsePreferences;
  const genres = [...new Set(allItems.flatMap((item) => item.genres))].sort((left, right) => left.localeCompare(right));
  const years = [...new Set(allItems.flatMap((item) => item.release_year ? [String(item.release_year)] : []))].sort((left, right) => Number(right) - Number(left));
  // Keep a saved filter visible even when the current library no longer has matching items.
  if (preferences.genre && !genres.includes(preferences.genre)) genres.push(preferences.genre);
  if (preferences.year && !years.includes(preferences.year)) years.push(preferences.year);
  return `<form id="library-browse-form" class="browse-toolbar" data-nav-section="library-filters" data-nav-label="Sort and filter" ${state.browseFiltersOpen ? '' : 'hidden'}>
    ${browseSelect('sort', 'Sort', preferences.sort, [['title', 'Title A–Z'], ['recent', 'Last modified'], ['year', 'Release year']])}
    ${browseSelect('watched', 'Watch status', preferences.watched, [['all', 'All titles'], ['unwatched', 'Unwatched'], ['in-progress', 'In progress'], ['watched', 'Watched']])}
    ${browseSelect('genre', 'Genre', preferences.genre, [['', 'All genres'], ...genres.map((genre): [string, string] => [genre, genre])])}
    ${browseSelect('year', 'Year', preferences.year, [['', 'All years'], ...years.map((year): [string, string] => [year, year])])}
    ${browseSelect('density', 'Card size', preferences.density, [['comfortable', 'Medium'], ['compact', 'Small'], ['spacious', 'Large']])}
    <button type="button" class="secondary-button" id="reset-library-filters">${renderButtonContent('Reset', 'x')}</button>
  </form>`;
}

export function visibleLibraryItems(): MediaItemSummary[] {
  return browseItems(filteredTopLevelLibraryItems(), state.browsePreferences);
}

function renderEmptyLibrary(library?: MediaLibrary): string {
  if (filteredTopLevelLibraryItems().length) {
    return '<div class="empty-state">No titles match these filters. Try another selection or reset the filters.</div>';
  }
  if (state.browseFilter) {
    return `<div class="empty-state">No items matched the current ${escapeHtml(state.browseFilter.kind)} filter.</div>`;
  }
  if (library?.status === 'never_scanned') {
    return '<div class="empty-state">This library has not been scanned yet. Titles will appear after its first scan.</div>';
  }
  if (library?.status && library.status !== 'available') {
    return `<div class="empty-state">This library is not ready yet: ${escapeHtml(libraryStatusLabel(library.status))}.</div>`;
  }
  return '<div class="empty-state">No titles are available in this library yet.</div>';
}

function renderLibraryPagination(count: number, page: number, lastPage: number): string {
  if (lastPage <= 0) return '';
  const start = page * LIBRARY_PAGE_SIZE;
  return `<nav class="browse-pagination" aria-label="Library pages">
    <button type="button" class="secondary-button" data-library-page="${page - 1}" ${page === 0 ? 'disabled' : ''}>${renderButtonContent('Previous', 'chevron-left')}</button>
    <span>${start + 1}–${Math.min(count, start + LIBRARY_PAGE_SIZE)} of ${count} titles · Page ${page + 1} of ${lastPage + 1}</span>
    <button type="button" class="secondary-button" data-library-page="${page + 1}" ${page === lastPage ? 'disabled' : ''}>${renderButtonContent('Next', 'chevron-right')}</button>
  </nav>`;
}

export function renderLibraryTab(): string {
  const items = visibleLibraryItems();
  const library = activeLibrary();
  const refreshProgress = library ? metadataRefreshActivityProgressForLibrary(library.id) : undefined;
  const isSpecificLibrary = state.route.page === 'home' && typeof state.route.libraryId === 'number';
  const browseFilterKind = state.browseFilter ? browseFilterKindLabel(state.browseFilter.kind) : '';
  const filterCount = activeBrowseFilterCount(state.browsePreferences);
  const filterSuffix = filterCount === 1 ? '' : 's';
  const filterStatus = filterCount ? `${filterCount} active filter${filterSuffix}` : 'No filters applied';
  const filterLabel = filterCount ? `Filters · ${filterCount} active` : 'Filters';

  let content = '';
  if (state.libraryItemsLoading && !state.libraryItems.length) {
    content = renderLoadingCards('Loading library items');
  } else if (!items.length) {
    content = renderEmptyLibrary(library);
  } else {
    const lastPage = Math.max(0, Math.ceil(items.length / LIBRARY_PAGE_SIZE) - 1);
    const page = Math.min(lastPage, state.libraryPage);
    const start = page * LIBRARY_PAGE_SIZE;
    content = `<div class="item-grid library-item-grid density-${state.browsePreferences.density}">${items.slice(start, start + LIBRARY_PAGE_SIZE).map(renderItemCard).join('')}</div>
      ${renderLibraryPagination(items.length, page, lastPage)}`;
  }

  return `
    <section class="browse-section" id="library-browse-root">
      <div class="shelf-header browse-section-header" data-nav-section="library-tools" data-nav-label="Library controls">
        <h3>${isSpecificLibrary ? escapeHtml(library?.name ?? 'Library') : 'All libraries'}</h3>
        <div class="browse-header-actions">
          ${refreshProgress ? `<span class="tag warning" role="status">Refreshing metadata ${refreshProgress.completed}/${refreshProgress.total}${refreshProgress.failed ? ` · ${refreshProgress.failed} failed` : ''}</span>` : ''}
          <span role="status">${items.length} title${items.length === 1 ? '' : 's'}</span>
          <button type="button" class="secondary-button browse-filter-toggle ${filterCount ? 'has-filters' : ''}" id="toggle-library-filters" aria-expanded="${state.browseFiltersOpen}" aria-controls="library-browse-form" aria-label="Filters: ${filterStatus}" title="${filterStatus}">${renderButtonContent(filterLabel, 'sliders-horizontal')}</button>
        </div>
      </div>
      ${renderLibraryToolbar()}
      ${state.browseFilter ? `
        <div class="active-filter-bar">
          <span class="tag success">${escapeHtml(browseFilterKind)}</span>
          <strong>${escapeHtml(state.browseFilter.label)}</strong>
          <button type="button" class="secondary-button" id="clear-browse-filter">${renderButtonContent('Clear filter', 'x')}</button>
        </div>
      ` : ''}
      <div data-nav-section="items" data-nav-label="Library titles">${content}</div>
    </section>
  `;
}

export function renderCollectionsTab(): string {
  const collections = collectionSummaries();
  if (!collections.length) {
    if (state.isLoading) return renderLoadingCards('Loading collections');
    return '<div class="empty-state">No linked collection data is available yet for this library.</div>';
  }

  return `
    <section class="item-grid" data-nav-section="collections" data-nav-label="Collections" aria-label="Collections">
      ${collections.map((collection) => {
        const posterUrl = collection.artwork_url ?? collection.backdrop_url;
        return `
          <button
            type="button"
            class="media-card collection-browse-card"
            data-collection-filter="${escapeHtml(collection.id)}"
            data-preview-collection-id="${escapeHtml(collection.id)}"
          >
            <span class="media-card-art collection" style="${posterUrl ? `background-image: url('${escapeHtml(resolveApiUrl(posterUrl))}');` : ''}">
              <span class="media-card-kind-row">
                <span class="media-card-kind">${renderIcon('layers', 'card-icon')}</span>
                <span class="media-card-duration">${collection.item_count} title${collection.item_count === 1 ? '' : 's'}</span>
              </span>
            </span>
            <span class="media-card-title">${escapeHtml(collection.name)}</span>
          </button>
        `;
      }).join('')}
    </section>
  `;
}

export function renderPlaylistsTab(): string {
  return `
    <section class="category-grid" data-nav-section="playlists" data-nav-label="Playlists" aria-label="Playlists">
      <button
        type="button"
        class="category-card panel filter-card-button"
        data-playlist-filter="Playlists"
      >
        <div class="category-card-header">
          <strong>Playlists</strong>
          <span class="tag">0 titles</span>
        </div>
        <p class="muted">Playlist creation is planned. Items will appear here when playlists are available.</p>
      </button>
    </section>
  `;
}

export function renderCategoriesTab(): string {
  const categories = categorySummaries();
  if (!categories.length) {
    if (state.libraryItemsLoading) return renderLoadingCards('Loading genres', 4, 'tiles');
    return '<div class="empty-state">No genre metadata is available yet for the current library.</div>';
  }

  return `
    <section class="category-grid" data-nav-section="categories" data-nav-label="Categories" aria-label="Categories">
      ${categories.map((category) => `
        <button
          type="button"
          class="category-card panel filter-card-button"
          data-category-filter="${escapeHtml(category.genre)}"
          ${category.items[0] ? `data-preview-item-id="${category.items[0].id}"` : ''}
        >
          <div class="category-card-header">
            <strong>${escapeHtml(category.genre)}</strong>
            <span class="tag">${category.count} title${category.count === 1 ? '' : 's'}</span>
          </div>
          <p class="muted">${escapeHtml(category.items.slice(0, 3).map((item) => item.display_title).join(' · ') || 'No titles yet')}</p>
        </button>
      `).join('')}
    </section>
  `;
}

export function renderHomeTabContent(): string {
  if (state.route.page === 'browse-detail') {
    return renderBrowseDetailPage();
  }

  if (state.browseFilter) {
    return renderBrowseFilterDetail();
  }

  if (state.showFullSearchResults && state.searchQuery.trim()) {
    return renderSearchResults();
  }

  switch (state.homeTab) {
    case 'library':
      return renderLibraryTab();
    case 'collections':
      return renderCollectionsTab();
    case 'playlists':
      return renderPlaylistsTab();
    case 'categories':
      return renderCategoriesTab();
    default:
      return renderShelfStack();
  }
}

export function renderHomePage(): string {
  if (state.route.page === 'browse-detail') {
    return `
      ${renderHomeNavbar()}
      ${renderBrowseDetailPage()}
    `;
  }

  return `
    <div class="home-pinned-header">
      ${renderHomeNavbar()}
      ${renderHomeFeature()}
    </div>
    <section class="shelf-stack panel page-panel">${renderHomeTabContent()}</section>
  `;
}

export function renderSearchPopover(): string {
  if (!state.searchQuery.trim() || state.showFullSearchResults) {
    return '';
  }

  if (state.searchLoading) return '<div class="search-popover panel"><div class="search-loading-status" role="status">Searching your library…</div></div>';

  if (!state.searchResults.length) {
    return '<div class="search-popover panel"><div class="empty-state tight">No library content matched the current search.</div></div>';
  }

  return `
    <div class="search-popover panel" data-nav-section="search-preview" data-nav-label="Search results">
      <div class="search-popover-header">
        <strong>Search results</strong>
        <span>${state.searchResults.length} match${state.searchResults.length === 1 ? '' : 'es'}</span>
      </div>
      <div class="search-results-list compact">
        ${state.searchResults.slice(0, 8).map((result) => renderSearchResultRow(result, true)).join('')}
      </div>
    </div>
  `;
}

export function renderHomeNavbar(): string {
  const library = activeLibrary();
  const libraryRefreshPending = library ? libraryHasActiveMetadataRefresh(library.id) : false;
  const libraryScanPending = library ? hasActiveLibraryScan(library.id) : hasActiveLibraryScan();
  const hasSearch = Boolean(state.searchQuery) || state.searchResults.length > 0 || state.showFullSearchResults;
  const searchToggleLabel = hasSearch ? 'Clear search' : 'Search';
  const searchButtonType = hasSearch ? 'button' : 'submit';
  const searchClearAttribute = hasSearch ? 'data-clear-search' : '';
  const searchIcon = hasSearch ? 'x' : 'search';
  const scanButtonDisabled = libraryScanPending ? 'disabled' : '';
  const refreshButtonDisabled = libraryRefreshPending ? 'disabled' : '';
  const libraryActionButtons = library && canManageServer()
    ? `
            <button type="button" class="icon-button secondary-button" id="scan-active-library" title="Scan library" aria-label="Scan library" ${scanButtonDisabled}>${renderIcon('folder-sync')}</button>
            <button type="button" class="icon-button secondary-button" id="refresh-active-library-metadata" title="Refresh metadata" aria-label="Refresh metadata" ${refreshButtonDisabled}>${renderIcon('database-zap')}</button>
            `
    : '';

  return `
    <header class="home-navbar" data-nav-section="browse-controls" data-nav-label="Browse and search">
      ${renderHomeTabs()}
      <div class="home-navbar-tools">
        <form id="search-form" class="search-form">
          <input id="search-input" name="search" type="search" value="${escapeHtml(state.searchQuery)}" placeholder="Search" autocomplete="off" />
          <button
            id="search-toggle"
            type="${searchButtonType}"
            class="icon-button search-toggle-button"
            title="${searchToggleLabel}"
            aria-label="${searchToggleLabel}"
            ${searchClearAttribute}
          >${renderIcon(searchIcon)}</button>
          </form>
          ${libraryActionButtons}
      </div>
      ${renderSearchPopover()}
    </header>
  `;
}
