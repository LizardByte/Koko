/** Renders private subscriptions, directory discovery, and publisher podcast metadata. */
import './podcasts.css';
import { getArtworkUrl } from '../api';
import type { DirectoryPodcast, MediaItemDetail, MediaItemSummary } from '../api';
import { escapeHtml, formatDuration, formatTimestamp } from './format';
import { renderButtonContent, renderIcon } from './ui';
import { state } from './state';

/** Only public HTTP(S) publisher links can appear in generated markup. */
export function podcastLink(value: string | null | undefined): string | undefined {
  if (!value) return undefined;
  try { const url = new URL(value); return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password ? url.href : undefined; }
  catch { return undefined; }
}

function cover(item: { artwork_url?: string | null }, title: string, itemId?: number): string {
  const url = itemId === undefined ? podcastLink(item.artwork_url) : getArtworkUrl(itemId);
  return url ? `<img src="${escapeHtml(url)}" alt="${escapeHtml(title)}" loading="lazy" referrerpolicy="no-referrer"/>` : renderIcon('mic-2');
}
function link(url: string | null | undefined, label: string): string {
  const safe = podcastLink(url);
  return safe ? `<a href="${escapeHtml(safe)}" target="_blank" rel="noopener noreferrer">${escapeHtml(label)}</a>` : '';
}
function card(item: DirectoryPodcast, index: number): string {
  const subscribed = state.podcasts?.items.some(show => show.podcast?.feed_url === item.feed_url);
  const label = subscribed ? 'Added' : 'Add';
  return `<article class="podcast-card"><div class="podcast-cover">${cover(item, item.title)}</div><div class="podcast-card-copy"><h3>${escapeHtml(item.title)}</h3><p class="muted">${escapeHtml(item.author ?? '')}</p>${link(item.apple_url, 'Apple Podcasts')}</div><button class="secondary-button" type="button" data-podcast-add="${index}" ${state.podcasts?.busy || subscribed ? 'disabled' : ''}>${renderButtonContent(label, subscribed ? 'check' : 'plus')}</button></article>`;
}
function subscription(item: MediaItemSummary): string {
  return `<article class="podcast-card"><button class="podcast-cover" type="button" data-item-id="${item.id}" aria-label="${escapeHtml(item.display_title)}">${cover(item.podcast ?? {}, item.display_title, item.id)}</button><div class="podcast-card-copy"><button class="podcast-title-button" type="button" data-item-id="${item.id}"><h3>${escapeHtml(item.display_title)}</h3></button><p class="muted">${escapeHtml(item.podcast?.author ?? '')}</p></div><button class="secondary-button" type="button" data-podcast-remove="${item.id}" ${state.podcasts?.busy ? 'disabled' : ''}>Remove</button></article>`;
}

/** Directory outages leave subscriptions and manual RSS entry usable. */
export function renderPodcastPage(): string {
  const data = state.podcasts?.owner === state.bootstrap?.current_user?.id ? state.podcasts : undefined;
  return `<section class="podcast-page" data-nav-section="podcasts" data-nav-label="Podcasts">
    <div class="section-heading"><div><span class="eyebrow">Listen</span><h1>Podcasts</h1><p class="muted">Discover a show and add it to your own collection.</p></div></div>
    <section class="panel page-panel"><div class="section-heading"><h2>Your podcasts</h2></div><div class="podcast-grid">${data?.items.map(subscription).join('') || '<p class="muted">No podcasts added yet. Choose a show below or add its feed URL.</p>'}</div></section>
    <section class="panel page-panel"><h2>Add a feed</h2><form id="podcast-feed-form" class="podcast-form"><label class="sr-only" for="podcast-feed">Podcast RSS feed URL</label><input id="podcast-feed" name="feed" type="url" required maxlength="8192" placeholder="https://example.com/podcast.xml"/><button type="submit" ${data?.busy ? 'disabled' : ''}>${renderButtonContent('Add podcast', 'plus')}</button></form><p class="muted">Use the show's RSS feed URL. No directory listing or API key is required.</p></section>
    <section class="panel page-panel"><div class="section-heading"><h2>${data?.query ? 'Search results' : 'Popular podcasts'}</h2></div>
    <form id="podcast-search-form" class="podcast-form"><label class="sr-only" for="podcast-search">Search podcasts</label><input id="podcast-search" name="query" type="search" maxlength="200" placeholder="Search podcasts" value="${escapeHtml(data?.query ?? '')}"/><label for="podcast-country">Country</label><input id="podcast-country" name="country" class="podcast-country" minlength="2" maxlength="2" pattern="[A-Za-z]{2}" required value="${escapeHtml(data?.country ?? 'us')}" aria-describedby="podcast-country-hint"/><button type="submit" ${data?.loading ? 'disabled' : ''}>${renderButtonContent('Search', 'search')}</button><button type="button" data-podcast-popular ${data?.loading ? 'disabled' : ''}>Popular</button></form><p class="muted" id="podcast-country-hint">Two-letter country code, such as US, CA, or GB. Directory: ${link('https://podcasts.apple.com/', 'Apple Podcasts')}.</p>
    ${data?.loading ? '<p role="status">Loading podcasts…</p>' : ''}${data?.error ? `<p role="alert">${escapeHtml(data.error)}</p>` : ''}
    <div class="podcast-grid">${data?.directory.map(card).join('') ?? ''}</div>${data && !data.loading && !data.error && !data.directory.length ? '<p class="muted">No podcasts found. Try another search or add the RSS feed above.</p>' : ''}</section></section>`;
}

/** Preserve serial story order; episodic shows present their newest publications first. */
export function orderedPodcastEpisodes(items: MediaItemSummary[], serial = false): MediaItemSummary[] {
  return [...items].sort((a, b) => serial
    ? (a.season_number ?? 0) - (b.season_number ?? 0) || (a.episode_number ?? Number.MAX_SAFE_INTEGER) - (b.episode_number ?? Number.MAX_SAFE_INTEGER) || (a.podcast?.published_at ?? 0) - (b.podcast?.published_at ?? 0) || a.id - b.id
    : (b.podcast?.published_at ?? 0) - (a.podcast?.published_at ?? 0) || b.id - a.id);
}
function listeningStatus(item: MediaItemSummary): string {
  if (item.playback_completed) return 'Played';
  return (item.playback_position_ms ?? 0) > 0 ? 'In progress' : '';
}
function paragraph(text: string | null | undefined, className = ''): string {
  return text ? `<p class="${className}">${escapeHtml(text)}</p>` : '';
}
function episode(item: MediaItemSummary): string {
  const resume = !item.playback_completed ? item.playback_position_ms ?? 0 : 0;
  const details = [item.podcast?.published_at ? formatTimestamp(item.podcast.published_at) : '', item.duration_ms ? formatDuration(item.duration_ms) : '', item.podcast?.episode_type, item.podcast?.explicit ? 'Explicit' : '', listeningStatus(item)].filter(Boolean).join(' · ');
  return `<article class="podcast-episode"><div class="podcast-card-copy"><button class="podcast-title-button" type="button" data-item-id="${item.id}"><h3>${escapeHtml(item.display_title)}</h3></button><p class="muted">${escapeHtml(details)}</p>${paragraph(item.overview, 'podcast-episode-overview')}</div><button type="button" class="primary-button" data-podcast-play="${item.id}" data-podcast-position="${resume}">${renderButtonContent(resume > 0 ? 'Resume' : 'Play', 'play')}</button></article>`;
}
function fact(label: string, value: string | null | undefined): string {
  return value ? `<div><dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd></div>` : '';
}
function seasonButton(item: MediaItemSummary): string {
  return `<button type="button" class="secondary-button" data-item-id="${item.id}">${escapeHtml(item.display_title)} · ${item.child_count} episodes</button>`;
}
function breadcrumb(item: MediaItemSummary): string {
  return `<span>›</span><button type="button" data-item-id="${item.id}">${escapeHtml(item.display_title)}</button>`;
}
function section(heading: string, contents: string): string {
  return contents ? `<section class="panel page-panel"><h2>${escapeHtml(heading)}</h2>${contents}</section>` : '';
}
function transcriptLink(entry: { url: string; language?: string | null }, index: number): string {
  const language = entry.language ? ' (' + entry.language + ')' : '';
  return link(entry.url, 'Transcript ' + (index + 1) + language);
}
function contentLabel(explicit?: boolean | null): string | undefined {
  if (explicit === null || explicit === undefined) return undefined;
  return explicit ? 'Explicit' : 'Clean';
}
function itemKind(item: MediaItemSummary): string {
  if (item.item_type === 'podcast') return 'Podcast';
  return item.item_type === 'podcast_season' ? 'Season' : 'Episode';
}
function details(item: MediaItemDetail): string {
  const metadata = item.podcast!;
  const facts = fact('Publisher', metadata.author) + fact('Language', metadata.language) + fact('Content', contentLabel(metadata.explicit))
    + fact('Show type', metadata.show_type) + fact('Episode type', metadata.episode_type)
    + fact('Published', metadata.published_at ? formatTimestamp(metadata.published_at) : undefined)
    + fact('Duration', item.duration_ms ? formatDuration(item.duration_ms) : undefined)
    + fact('Season', item.season_number?.toString()) + fact('Episode', item.episode_number?.toString())
    + fact('People', metadata.people.join(', ')) + fact('Copyright', metadata.copyright)
    + fact('Last checked', metadata.checked_at ? formatTimestamp(metadata.checked_at) : undefined);
  const links = link(metadata.website_url, 'Publisher website') + link(metadata.feed_url, 'RSS feed') + metadata.transcripts.map(transcriptLink).join('') + link(metadata.chapters_url, 'Episode chapters');
  return section('Details', `<dl class="podcast-facts">${facts}</dl><div class="podcast-links">${links}</div>`);
}

/** Podcast item pages use ordinary item navigation and independent account listening progress. */
export function renderPodcastItem(item: MediaItemDetail): string {
  const metadata = item.podcast!;
  const seasons = item.children.filter(child => child.item_type === 'podcast_season').sort((a, b) => (a.season_number ?? 0) - (b.season_number ?? 0));
  const episodes = orderedPodcastEpisodes(item.children.filter(child => child.item_type === 'podcast_episode'), metadata.show_type === 'serial');
  const seasonList = seasons.length ? section('Seasons', '<div class="podcast-season-list">' + seasons.map(seasonButton).join('') + '</div>') : '';
  const episodeList = section(seasons.length ? 'Other episodes' : 'Episodes', episodes.map(episode).join(''));
  const empty = !item.playable && !seasons.length && !episodes.length ? '<p class="muted">This feed has no playable episodes yet.</p>' : '';
  const warning = metadata.last_error ? paragraph('Last feed check failed: ' + metadata.last_error + '. You can still play saved episodes that the publisher makes available.', 'status-warning') : '';
  return `<section class="podcast-page podcast-item-page" data-item-page-id="${item.id}"><nav class="podcast-breadcrumbs" aria-label="Podcast hierarchy"><button type="button" data-nav-podcasts>Podcasts</button>${item.hierarchy.map(breadcrumb).join('')}</nav>
    <header class="podcast-hero"><div class="podcast-cover">${cover(metadata, item.display_title, item.id)}</div><div><span class="eyebrow">${itemKind(item)}</span><h1>${escapeHtml(item.display_title)}</h1><p class="muted">${escapeHtml(metadata.author ?? '')}</p><p>${escapeHtml(metadata.categories.join(' · '))}</p>${item.playable ? episode(item) : ''}<div class="podcast-form"><button class="secondary-button" type="button" data-podcast-refresh="${metadata.podcast_id}">Check for new episodes</button><button class="secondary-button" type="button" data-podcast-remove="${metadata.podcast_id}">Remove podcast</button></div></div></header>
    ${warning}${section(item.playable ? 'Show notes' : 'About', paragraph(item.overview, 'podcast-notes'))}
    ${seasonList}${episodeList}${empty}${details(item)}</section>`;
}
