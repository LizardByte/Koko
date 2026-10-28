/** Presents podcast discovery and episode lists using Koko's shared library and item components. */
import './podcasts.css';
import type { DirectoryPodcast, MediaItemDetail, MediaItemSummary, PodcastPlaybackOrder } from '../api';
import { escapeHtml, formatDuration, formatFileSize, formatTimestamp } from './format';
import { renderButtonContent, renderIcon } from './ui';
import { renderItemCard, renderItemFeature, renderShelfStack, renderGlobalSearchControl, renderSearchPopover, renderSearchResults } from './homeView';
import { renderAudioTrackRow } from './music';
import { activeMusicPlayback } from './musicPlaybackState';
import { state } from './state';

/** Restrict publisher links to web URLs without embedded login credentials. */
export function podcastLink(value: string | null | undefined): string | undefined {
  if (!value) return undefined;
  try { const url = new URL(value); return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password ? url.href : undefined; }
  catch { return undefined; }
}
function link(url: string | null | undefined, label: string): string {
  const safe = podcastLink(url);
  return safe ? `<a href="${escapeHtml(safe)}" target="_blank" rel="noopener noreferrer">${escapeHtml(label)}</a>` : '';
}
function directoryCard(item: DirectoryPodcast, index: number): string {
  const subscribed = state.podcasts?.items.some(show => show.podcast?.feed_url === item.feed_url);
  const artwork = podcastLink(item.artwork_url);
  const image = artwork ? `<img src="${escapeHtml(artwork)}" alt="" loading="lazy" referrerpolicy="no-referrer"/>` : renderIcon('mic-2');
  const label = subscribed ? 'Already added ' : 'Add ';
  const apple = link(item.apple_url, 'Open in Apple Podcasts');
  const menu = apple ? `<div class="music-song-menu podcast-directory-menu"><button type="button" class="secondary-button icon-only" popovertarget="podcast-directory-${index}" aria-label="More actions for ${escapeHtml(item.title)}">${renderButtonContent('', 'ellipsis')}</button><div popover id="podcast-directory-${index}">${apple}</div></div>` : '';
  return `<article class="podcast-directory-card"><div class="detail-art music-art">${image}<button type="button" class="podcast-cover-add icon-button" data-podcast-add="${index}" aria-label="${escapeHtml(label + item.title)}" title="${escapeHtml(label + item.title)}" ${state.podcasts?.busy || subscribed ? 'disabled' : ''}>${renderIcon(subscribed ? 'check' : 'plus')}</button>${menu}</div><h3>${escapeHtml(item.title)}</h3><p class="muted">${escapeHtml(item.author ?? '')}</p></article>`;
}
function feedForm(): string {
  if (!state.podcasts?.addFeedOpen) return '';
  return `<section class="panel page-panel podcast-add-panel"><div class="section-heading"><h3>Add a podcast feed</h3></div><form id="podcast-feed-form" class="form-row"><label>RSS or Atom feed URL<input name="feed" type="url" required maxlength="8192" placeholder="https://example.com/podcast.xml"/></label><button type="submit" ${state.podcasts.busy ? 'disabled' : ''}>${renderButtonContent('Add podcast', 'plus')}</button></form><p class="muted">Paste the publisher's public feed URL. No directory listing or API key is required.</p></section>`;
}
function discovery(): string {
  const data = state.podcasts;
  if (!data) return '';
  const error = data.error ? `<div class="empty-state" role="alert">${escapeHtml(data.error)}</div>` : '';
  const status = data.loading ? '<div class="empty-state" role="status">Loading podcasts…</div>' : '';
  return `<section class="panel page-panel shelf-stack"><div class="section-heading"><h2>${data.query ? 'Search results' : 'Popular podcasts'}</h2></div><form id="podcast-search-form" class="podcast-search-controls"><label class="sr-only" for="podcast-search">Search podcasts</label><input id="podcast-search" name="query" type="search" maxlength="200" placeholder="Search podcasts" value="${escapeHtml(data.query)}"/><label>Country<input name="country" class="podcast-country" minlength="2" maxlength="2" pattern="[A-Za-z]{2}" required value="${escapeHtml(data.country)}"/></label><button type="submit" class="secondary-button" ${data.loading ? 'disabled' : ''}>${renderButtonContent('Search', 'search')}</button><button type="button" class="secondary-button" data-podcast-popular ${data.loading ? 'disabled' : ''}>Popular</button></form><p class="muted">Choose a country such as US, CA, or GB. Directory: ${link('https://podcasts.apple.com/', 'Apple Podcasts')}.</p>${error}${status}<div class="item-grid podcast-library-grid">${data.directory.map(directoryCard).join('')}</div>${!data.loading && !data.error && !data.directory.length ? '<div class="empty-state">No podcasts found. Try another search or add an RSS feed.</div>' : ''}</section>`;
}

/** Reuse the library tabs, square artwork cards, and browsing grid from music. */
export function renderPodcastPage(): string {
  if (state.bootstrap?.podcasts_enabled === false) return '<div class="empty-state">Podcasts are disabled by your administrator.</div>';
  const data = state.podcasts?.owner === state.bootstrap?.current_user?.id ? state.podcasts : undefined;
  const tab = data?.tab ?? 'recommended';
  const tabs = ['recommended', 'library', 'discover'].map(value => `<button type="button" class="browse-tab-button ${tab === value ? 'active' : ''}" data-podcast-tab="${value}" aria-pressed="${tab === value}">${value.charAt(0).toUpperCase() + value.slice(1)}</button>`).join('');
  const library = data?.items.length ? `<div class="item-grid podcast-library-grid">${data.items.map(renderItemCard).join('')}</div>` : '<div class="empty-state">No podcasts added yet. Browse Discover or add a feed URL to start listening.</div>';
  const libraryPanel = `<section class="panel page-panel shelf-stack"><div class="section-heading item-row-heading"><h2>Podcasts <span class="item-row-count">· ${data?.items.length ?? 0}</span></h2></div>${library}</section>`;
  let content = libraryPanel;
  if (tab === 'discover' && data) content = discovery();
  if (tab === 'recommended' && data) content = renderPodcastRecommended();
  if (state.showFullSearchResults && state.searchQuery.trim()) content = renderSearchResults();
  return `<header class="home-navbar" data-nav-section="browse-controls" data-nav-label="Podcasts"><nav class="browse-tabs" aria-label="Podcast views">${tabs}</nav><div class="home-navbar-tools">${renderGlobalSearchControl()}<button type="button" class="secondary-button" data-podcast-feed-toggle>${renderButtonContent('Add RSS feed', 'plus')}</button></div>${renderSearchPopover()}</header><div class="podcast-library-content">${feedForm()}${content}</div>`;
}

/** Subscribed shows supply the shared hero and listening shelves; publisher covers remain available. */
export function renderPodcastRecommended(): string {
  const data = state.podcasts;
  const home = data?.home;
  const candidates = home?.shelves.flatMap(shelf => shelf.items) ?? [];
  const feature = candidates.find(item => item.id === state.homePreviewItemId) ?? candidates[0] ?? data?.items[0];
  const hero = feature ? renderItemFeature(feature, [...(data?.items ?? []), ...candidates]) : '';
  if (!data?.items.length) return '<div class="empty-state">No podcasts added yet. Browse Discover or add a feed URL to start listening.</div>';
  const shelves = home ? renderShelfStack(home) : `<div class="item-grid podcast-library-grid">${data.items.map(renderItemCard).join('')}</div>`;
  const error = data.error ? `<p class="status-warning" role="alert">${escapeHtml(data.error)}</p>` : '';
  return `${hero}${error}<section class="shelf-stack panel page-panel">${shelves}</section>`;
}

/** Sort chronologically without mutating the catalog, keeping undated episodes last. */
export function orderedPodcastEpisodes(items: MediaItemSummary[], order: PodcastPlaybackOrder = 'newest_first'): MediaItemSummary[] {
  return [...items].sort((left, right) => {
    const a = left.podcast?.published_at, b = right.podcast?.published_at;
    const known = Number(b !== null && b !== undefined) - Number(a !== null && a !== undefined);
    if (known) return known;
    const ascending = (a ?? 0) - (b ?? 0) || (left.season_number ?? 0) - (right.season_number ?? 0)
      || (left.episode_number ?? 0) - (right.episode_number ?? 0) || left.id - right.id;
    return order === 'newest_first' ? -ascending : ascending;
  });
}

/** Small download labels fit beside publication dates in the shared audio rows. */
export function podcastDownloadLabel(item: MediaItemSummary): string {
  const download = item.podcast?.download;
  if (!download) return 'Downloads on first play';
  return { queued: 'Download queued', downloading: 'Downloading', ready: 'Downloaded', failed: 'Download failed' }[download.state];
}
/** Use compact icons in episode lists while retaining clear tooltips and screen-reader labels. */
export function renderPodcastDownloadIcon(item: MediaItemSummary): string {
  const state = item.podcast?.download?.state;
  const label = podcastDownloadLabel(item);
  let icon = renderIcon('cloud-download');
  if (state === 'ready') icon = renderIcon('hard-drive-download');
  if (state === 'downloading' || state === 'queued') icon = renderIcon('refresh-cw');
  if (state === 'failed') icon = renderIcon('triangle-alert');
  return `<span class="podcast-download-status ${state === 'ready' ? 'is-downloaded' : ''}" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}">${icon}</span>`;
}

/** Render one episode with accessible download availability and shared audio controls. */
export function episodeRow(item: MediaItemSummary, chronologicalNumber: number): string {
  const playback = activeMusicPlayback();
  const isCurrent = playback?.item.id === item.id && !playback.ended;
  const resume = item.playback_completed ? 0 : item.playback_position_ms ?? 0;
  const subtitle = [item.podcast?.published_at ? formatTimestamp(item.podcast.published_at) : '', item.podcast?.episode_type].filter(Boolean).join(' · ');
  const actions = `<button type="button" data-item-id="${item.id}">Episode details</button><button type="button" data-podcast-play="${item.id}" data-podcast-position="0">Play from beginning</button>`;
  return renderAudioTrackRow(item, { number: String(item.episode_number ?? chronologicalNumber), menuId: `podcast-${item.id}`, isCurrent, cover: true, playAttributes: `data-podcast-play="${item.id}" data-podcast-position="${resume}"`, subtitle, actions, warnings: renderPodcastDownloadIcon(item) });
}

/** Use TV season cards and music track rows within the ordinary shared item page. */
export function renderPodcastContents(item: MediaItemDetail): string {
  const seasons = item.children.filter(child => child.item_type === 'podcast_season').sort((left, right) => {
    const a = left.season_number ?? left.podcast?.season_year ?? Number.MAX_SAFE_INTEGER;
    const b = right.season_number ?? right.podcast?.season_year ?? Number.MAX_SAFE_INTEGER;
    const known = Number(b !== Number.MAX_SAFE_INTEGER) - Number(a !== Number.MAX_SAFE_INTEGER);
    if (known) return known;
    const ascending = a - b || left.id - right.id;
    return -ascending;
  });
  const episodes = orderedPodcastEpisodes(item.children.filter(child => child.item_type === 'podcast_episode'));
  const total = item.item_type === 'podcast_season' ? item.child_count : episodes.length;
  const heading = seasons.length ? 'Seasons' : 'Episodes';
  const body = seasons.length ? `<div class="item-grid hierarchy-item-grid podcast-library-grid">${seasons.map(renderItemCard).join('')}</div>` : `<div class="music-track-list">${episodes.map((episode, index) => episodeRow(episode, total - index)).join('')}</div>`;
  if (!seasons.length && !episodes.length) {
    if (item.playable) return '';
    return '<div class="empty-state">This feed has no episodes yet.</div>';
  }
  const pageStatus = state.podcastEpisodeLoading?.itemId === item.id ? state.podcastEpisodeLoading : undefined;
  const pageError = pageStatus?.error ? `<span role="alert">${escapeHtml(pageStatus.error)}</span>` : '';
  const loadMoreLabel = pageStatus?.error ? 'Retry loading episodes' : 'Load more episodes';
  const paging = !seasons.length && episodes.length < total ? `<div class="podcast-page-sentinel" data-podcast-more="${item.id}"><button type="button" class="secondary-button" data-podcast-load-more="${item.id}">${loadMoreLabel}</button>${pageError}<span class="muted" role="status">${episodes.length} of ${total} episodes</span></div>` : '';
  return `<section class="panel page-panel item-section" data-nav-section="podcast-episodes" data-nav-label="${heading}"><div class="section-heading item-row-heading"><h3>${heading} <span class="item-row-count">· ${seasons.length || total}</span></h3></div>${body}${paging}</section>`;
}

/** Each subscriber chooses an independent order for the entire podcast, across seasons and years. */
export function renderPodcastControls(item: MediaItemDetail): string {
  const root = item.podcast!.podcast_id;
  const order = item.podcast!.playback_order ?? 'oldest_first';
  return `<div class="podcast-order-control"><label>Playback order<select data-podcast-order="${root}" aria-label="Podcast playback order"><option value="oldest_first" ${order === 'oldest_first' ? 'selected' : ''}>Oldest to newest</option><option value="newest_first" ${order === 'newest_first' ? 'selected' : ''}>Newest to oldest</option></select></label></div>`;
}

/** Podcast-specific maintenance belongs in the same More menu as other media actions. */
export function renderPodcastActions(item: MediaItemDetail): string {
  const id = item.podcast!.podcast_id;
  return `<button type="button" class="secondary-button" data-podcast-refresh="${id}">${renderButtonContent('Check for new episodes', 'refresh-cw')}</button><button type="button" class="secondary-button danger-button" data-podcast-remove="${id}">${renderButtonContent('Remove podcast', 'trash-2')}</button>`;
}
function fact(label: string, value?: string | null): string {
  return value ? `<div class="item-fact"><span class="label">${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>` : '';
}
function transcript(entry: { url: string; language?: string | null }, index: number): string {
  const language = entry.language ? ' (' + entry.language + ')' : '';
  return link(entry.url, 'Transcript ' + (index + 1) + language);
}

/** Publisher details share the item disclosure and never expose internal download paths. */
export function renderPodcastDetails(item: MediaItemDetail): string {
  const metadata = item.podcast!;
  const facts = fact('Publisher', metadata.author) + fact('Language', metadata.language) + fact('Show type', metadata.show_type)
    + fact('Episode type', metadata.episode_type) + fact('Published', metadata.published_at ? formatTimestamp(metadata.published_at) : undefined)
    + fact('Season', item.season_number?.toString()) + fact('Year', metadata.season_year?.toString()) + fact('Episode', item.episode_number?.toString())
    + fact('Duration', item.duration_ms ? formatDuration(item.duration_ms) : undefined) + fact('Participants', metadata.people.join(', '))
    + fact('Copyright', metadata.copyright) + fact('Last checked', metadata.checked_at ? formatTimestamp(metadata.checked_at) : undefined);
  const downloadSize = metadata.download?.size_bytes ? formatFileSize(metadata.download.size_bytes) : undefined;
  const localCopy = item.playable ? fact('Local copy', podcastDownloadLabel(item)) + fact('Download size', downloadSize) : '';
  const links = link(metadata.website_url, 'Publisher website') + link(metadata.feed_url, 'RSS feed') + metadata.transcripts.map(transcript).join('') + link(metadata.chapters_url, 'Episode chapters');
  const error = metadata.download?.error || metadata.last_error;
  const warning = error ? `<p class="status-warning" role="alert">${escapeHtml(error)}</p>` : '';
  return `<details class="item-management" id="item-management" ${state.itemManagementOpen ? 'open' : ''} data-nav-section="management" data-nav-label="Media details"><summary>${renderButtonContent('Media details', 'settings')}${renderIcon('chevron-down', 'disclosure-chevron')}</summary><div class="item-management-body"><div class="item-fact-list">${facts}${localCopy}</div>${warning}<div class="podcast-publisher-links">${links}</div></div></details>`;
}
