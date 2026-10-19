import { renderAlbumTracks } from './music';
/** Renders item detail, metadata search, person detail, and credit trays. */
import type {
  ItemMetadataMatch,
  ItemMetadataPerson,
  MediaItemDetail,
  MediaItemExtra,
  MediaItemSummary,
  MediaPlaybackTarget,
  MetadataPersonItemCredit,
  MetadataProviderStatus,
} from '../api';
import { getPersonImageUrl, resolveApiUrl } from '../api';
import { escapeHtml, formatBitRate, formatDuration, formatFileSize, formatTimestamp } from './format';
import { normalizedMetadataLanguages } from './formUtils';
import { extractYouTubeVideoId } from './youtube';
import { mediaExtraDurationLabel, mediaExtraTitle, mediaExtraTypeLabel } from './mediaExtras';
import { currentThemeSongYouTubeTarget, currentTrailerOptions } from './mediaTargets';
import { itemHasActiveMetadataRefresh, itemIsMetadataPending } from './activities';
import { resumablePlaybackPositionMs } from './playbackProgress';
import { renderLoadingDetail } from './loadingView';
import { renderRating } from './ratingView';
import { itemArtwork, renderEpisodeNavigation, renderSeasonTabs, renderTvChildren, renderTvEpisodeCard, renderTvTitle, showForItem, tvPlaybackLabel, tvRestartTargets } from './tvItemView';
import { canManageUsers } from './auth';
import { providerAttributionLogo, providerDisplayName } from './providers';
import { state } from './state';
import type { PersonCreditGroup, TrailerOption } from './types';
import {
  activeLibrary,
  activeLibrarySettings,
  backNavigationTarget,
  canManuallyLinkMetadata,
  selectedItemCollectionRails,
} from './selectors';
import {
  renderButtonContent,
  renderCollapsibleText,
  renderIcon,
} from './ui';
import { missingItemDetailBadgeMarkup, renderItemCard, renderMetadataRefreshBadge, renderPlaybackTargetButton, renderWatchProgress } from './homeView';

export function renderMetadataSearchResults(): string {
  const selectedItem = state.selectedItem;
  if (!selectedItem) {
    return '';
  }

  if (!state.metadataSearchResults.length) {
    return '<div class="empty-state tight">Search metadata providers to link rich metadata and artwork.</div>';
  }

  return state.metadataSearchResults
    .map((result) => `
      <article class="metadata-search-card">
        ${result.artwork_url ? `<img class="metadata-search-poster" src="${escapeHtml(resolveApiUrl(result.artwork_url))}" alt="" loading="lazy" />` : ''}
        <div>
          <strong>${escapeHtml(result.title)}</strong>
          <p>${escapeHtml(result.overview ?? 'No overview available.')}</p>
          <div class="metadata-match-meta">
            ${renderMetadataSearchProviderAttribution(result.provider_id)}
            <span>${result.release_year ?? 'Unknown year'}</span>
            <span>${escapeHtml(result.media_type)}</span>
            ${result.track_count ? `<span>${result.track_count} tracks${selectedItem.item_type === 'album' && selectedItem.child_count ? ` · ${result.track_count === selectedItem.child_count ? 'matches your' : 'your album has'} ${selectedItem.child_count} tracks` : ''}</span>` : ''}
            ${typeof result.score === 'number' ? `<span>${Math.round(result.score * 100)}% match</span>` : ''}
          </div>
        </div>
        <button
          type="button"
          class="secondary-button"
          data-link-metadata="${selectedItem.id}:${escapeHtml(result.provider_id)}:${escapeHtml(result.external_id)}:${escapeHtml(result.media_type)}"
        >
          ${renderButtonContent('Link', 'link-2')}
        </button>
      </article>
    `)
    .join('');
}

export function selectedItemMetadataProviderOptions(): MetadataProviderStatus[] {
  const itemType = state.selectedItem?.item_type;
  const libraryKind = activeLibrary()?.kind ?? libraryKindForItemType(itemType);
  return (state.selectedItemMetadata?.providers ?? state.metadataProviders)
    .filter((provider) => provider.role !== 'secondary')
    .filter((provider) => provider.configured && provider.implemented)
    .filter((provider) => !libraryKind || provider.supported_kinds.includes(libraryKind));
}

function libraryKindForItemType(itemType: string | undefined): string | undefined {
  if (itemType === 'show') {
    return 'shows';
  }
  if (['artist', 'album', 'track'].includes(itemType ?? '')) return 'music';
  if (itemType === 'movie') {
    return 'movies';
  }
  return undefined;
}

export function defaultMetadataSearchProviderIds(): string[] {
  const providers = selectedItemMetadataProviderOptions();
  const providerIds = new Set(providers.map((provider) => provider.id));
  const libraryProviderIds = activeLibrary()?.metadata_providers
    ?? activeLibrarySettings()?.metadata_providers;
  const selectedLibraryProviders = (libraryProviderIds ?? [])
    .filter((providerId) => providerIds.has(providerId));
  return libraryProviderIds ? selectedLibraryProviders : providers.map((provider) => provider.id);
}

export function selectedItemDefaultMetadataTitle(): string {
  return state.selectedItem?.display_title.trim()
    || state.selectedItemMetadata?.matches[0]?.title?.trim()
    || '';
}

export function selectedItemDefaultMetadataYear(): string {
  const year = state.selectedItem?.release_year ?? state.selectedItemMetadata?.matches[0]?.release_year;
  return typeof year === 'number' ? String(year) : '';
}

export function defaultMetadataSearchLanguage(): string {
  const library = activeLibrary();
  const librarySettings = activeLibrarySettings();
  const metadataLanguageMode = library?.metadata_language_mode ?? librarySettings?.metadata_language_mode;
  const metadataLanguages = library?.metadata_languages ?? librarySettings?.metadata_languages;
  if (metadataLanguageMode === 'manual') {
    return normalizedMetadataLanguages(metadataLanguages)[0] ?? 'en-US';
  }
  return state.bootstrap?.current_user?.preferred_metadata_languages?.[0]
    ?? state.metadataProviders.find((provider) => provider.configured)?.language
    ?? 'en-US';
}

export function renderMetadataSearchProviderAttribution(providerId: string): string {
  const label = providerDisplayName(providerId);
  const logoUrl = providerAttributionLogo(providerId);
  if (!logoUrl) {
    return `<span>${escapeHtml(label)}</span>`;
  }
  return `<img class="metadata-attribution-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(label)}" title="${escapeHtml(label)}" loading="lazy" />`;
}

export function renderMetadataSearchProviderControls(): string {
  const providers = selectedItemMetadataProviderOptions();
  if (!providers.length) {
    return '';
  }

  const selectedProviders = state.metadataSearchProviders.length
    ? state.metadataSearchProviders
    : defaultMetadataSearchProviderIds();

  return `
    <div class="metadata-provider-picker">
      ${providers.map((provider) => `
        <label class="checkbox-inline">
          <input
            name="metadataSearchProvider"
            type="checkbox"
            value="${escapeHtml(provider.id)}"
            ${selectedProviders.includes(provider.id) ? 'checked' : ''}
          />
          <span>${escapeHtml(provider.display_name)}</span>
        </label>
      `).join('')}
    </div>
  `;
}

export function renderLinkedMetadataSummary(): string {
  if (state.itemMetadataLoading && !state.selectedItemMetadata) return '<div class="empty-state tight" role="status">Loading linked metadata…</div>';
  const matches = state.selectedItemMetadata?.matches ?? [];
  const linkedMatch = matches.find((match) => match.relation_kind === 'primary') ?? matches[0];
  if (!linkedMatch) {
    return '<div class="empty-state tight">No external metadata is linked yet.</div>';
  }

  const metadataRefreshPending = itemIsMetadataPending(state.selectedItem);
  const metadataRefreshActive = itemHasActiveMetadataRefresh(state.selectedItem);
  let refreshStateLabel = 'Up to date';
  if (metadataRefreshActive) {
    refreshStateLabel = 'Refreshing';
  } else if (metadataRefreshPending || linkedMatch.refresh_state === 'pending') {
    refreshStateLabel = 'Pending without worker';
  } else if (linkedMatch.refresh_state === 'error') {
    refreshStateLabel = 'Refresh failed';
  }
  let refreshStateClass = '';
  if (metadataRefreshPending || linkedMatch.refresh_state === 'pending') {
    refreshStateClass = 'warning';
  } else if (linkedMatch.refresh_state === 'error') {
    refreshStateClass = 'danger-tag';
  }
  const providersById = new Map(
    (state.selectedItemMetadata?.providers ?? state.metadataProviders).map((provider) => [provider.id, provider]),
  );
  const contributingProviderIds = [
    linkedMatch.provider_id,
    ...matches.map((match) => match.provider_id).filter((providerId) => providerId !== linkedMatch.provider_id),
  ].filter((providerId, index, providerIds) => providerIds.indexOf(providerId) === index);
  const providerTags = contributingProviderIds
    .map((providerId) => {
      const className = providerId === linkedMatch.provider_id ? 'tag success' : 'tag';
      return `<span class="${className}">${escapeHtml(providerId)}</span>`;
    })
    .join('');
  const attributions = contributingProviderIds
    .map((providerId) => providersById.get(providerId))
    .filter((provider): provider is MetadataProviderStatus => Boolean(provider?.attribution_text))
    .map((provider) => {
      const logoUrl = providerAttributionLogo(provider.id);
      const logoMarkup = logoUrl ? `<img src="${escapeHtml(logoUrl)}" alt="" loading="lazy" />` : '';
      return `<a class="metadata-attribution" href="${escapeHtml(provider.attribution_url)}" target="_blank" rel="noreferrer">${logoMarkup}${escapeHtml(provider.attribution_text)}</a>`;
    })
    .join('');

  return `
    <div class="metadata-current-link">
      ${providerTags}
      <span class="tag">${escapeHtml(linkedMatch.media_type ?? 'linked')}</span>
      <span class="tag ${refreshStateClass}">${escapeHtml(refreshStateLabel)}</span>
      ${linkedMatch.release_year ? `<span class="tag">${linkedMatch.release_year}</span>` : ''}
      ${linkedMatch.locale_key ? `<span class="tag">${escapeHtml(linkedMatch.locale_key)}</span>` : ''}
      <span class="metadata-current-copy">
        <strong>${escapeHtml(linkedMatch.title ?? linkedMatch.external_id)}</strong>
        <span class="muted">Last refreshed ${escapeHtml(formatTimestamp(linkedMatch.last_refreshed_at ?? linkedMatch.updated_at))}</span>
        ${attributions}
        ${linkedMatch.refresh_error ? `<span class="metadata-refresh-error">${escapeHtml(linkedMatch.refresh_error)}</span>` : ''}
      </span>
    </div>
  `;
}

export function selectedItemPeople(): ItemMetadataPerson[] {
  return state.selectedItemMetadata?.matches[0]?.people ?? [];
}

export function formatPersonDate(value?: string): string {
  if (!value) {
    return '';
  }

  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

export function personAgeLabel(birthday?: string, deathday?: string): string | undefined {
  if (!birthday) {
    return undefined;
  }
  const birthDate = new Date(`${birthday}T00:00:00`);
  const endDate = deathday ? new Date(`${deathday}T00:00:00`) : new Date();
  if (Number.isNaN(birthDate.getTime()) || Number.isNaN(endDate.getTime())) {
    return undefined;
  }
  let age = endDate.getFullYear() - birthDate.getFullYear();
  const birthdayThisYear = new Date(endDate.getFullYear(), birthDate.getMonth(), birthDate.getDate());
  if (endDate < birthdayThisYear) {
    age -= 1;
  }
  return deathday ? `${age} at death` : `${age} years old`;
}

export function renderPersonCredit(person: ItemMetadataPerson): string {
  let imageUrl: string | undefined;
  if (person.cached_image_path) {
    imageUrl = getPersonImageUrl(person.person_id);
  } else if (person.image_url) {
    imageUrl = resolveApiUrl(person.image_url);
  }
  const subtitle = person.character_name || person.role || person.department || '';
  return `
    <button class="person-card" type="button" data-person-id="${person.person_id}">
      <span class="person-card-art ${imageUrl ? 'has-image' : ''}" ${imageUrl ? `style="background-image: url('${escapeHtml(imageUrl)}');"` : ''}>
        ${imageUrl ? '' : `<span>${escapeHtml(person.name.slice(0, 1).toUpperCase())}</span>`}
      </span>
      <span class="person-card-title">${escapeHtml(person.name)}</span>
      ${subtitle ? `<span class="person-card-subtitle">${escapeHtml(subtitle)}</span>` : ''}
    </button>
  `;
}

export function renderPeopleRail(): string {
  const people = selectedItemPeople();
  if (!people.length) {
    return '';
  }

  const cast = people.filter(isCastCredit);
  const crew = people.filter((person) => !isCastCredit(person));
  return renderPeopleRow('Cast', cast, 'people') + renderPeopleRow('Crew', crew, 'crew');
}

/** Cast and crew share single rows of complete cards and separate navigation sections. */
function renderPeopleRow(title: string, people: ItemMetadataPerson[], section: string): string {
  if (!people.length) return '';
  return `<section class="panel page-panel item-section item-people-section" data-nav-section="${section}" data-nav-label="${title}">
    <div class="section-heading item-row-heading"><h3>${title} <span class="item-row-count">· ${people.length}</span></h3></div>
    <div class="shelf-row-shell item-row-shell">
      <button type="button" class="shelf-scroll-button" data-shelf-scroll="${section}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
      <div class="people-row item-responsive-row" data-item-row="people" data-shelf-row="${section}">${people.map(renderPersonCredit).join('')}</div>
      <button type="button" class="shelf-scroll-button" data-shelf-scroll="${section}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
    </div>
  </section>`;
}

/** Treat unclassified people as cast so incomplete provider data stays visible. */
function isCastCredit(person: ItemMetadataPerson): boolean {
  return Boolean(person.character_name)
    || ['cast', 'acting'].includes(person.department?.toLowerCase() ?? '')
    || person.role?.toLowerCase() === 'actor'
    || (!person.department && !person.role);
}

export function selectedItemExtras(): MediaItemExtra[] {
  return (state.selectedItem?.extras ?? []).filter((extra) => Boolean(extra.url?.trim()));
}

export function mediaExtraThumbnailUrl(extra: MediaItemExtra): string | undefined {
  if (extra.thumbnail_url?.trim()) {
    return resolveApiUrl(extra.thumbnail_url.trim());
  }

  const videoId = extractYouTubeVideoId(extra.url);
  return videoId ? `https://i.ytimg.com/vi/${videoId}/hqdefault.jpg` : undefined;
}

export function renderMediaExtraCard(extra: MediaItemExtra, index: number): string {
  const title = mediaExtraTitle(extra);
  const typeLabel = mediaExtraTypeLabel(extra.extra_type);
  const durationLabel = mediaExtraDurationLabel(extra);
  const thumbnailUrl = mediaExtraThumbnailUrl(extra);
  const placeholderIcon = extra.extra_type === 'theme_song' ? 'music' : 'play';
  const thumbnailMarkup = thumbnailUrl
    ? `<img src="${escapeHtml(thumbnailUrl)}" alt="${escapeHtml(title)} thumbnail" loading="lazy" />`
    : renderIcon(placeholderIcon, 'media-extra-placeholder-icon');
  return `
    <button type="button" class="media-extra-card" data-play-extra-index="${index}" title="${escapeHtml(title)}">
      <span class="media-extra-thumbnail ${thumbnailUrl ? 'has-image' : ''}">
        ${thumbnailMarkup}
        <span class="media-extra-play-icon">${renderIcon('play', 'button-icon')}</span>
      </span>
      <span class="media-extra-title">${escapeHtml(title)}</span>
      <span class="media-extra-meta">
        <span>${escapeHtml(typeLabel)}</span>
        <span>${escapeHtml(durationLabel)}</span>
      </span>
    </button>
  `;
}

export function renderItemExtrasRail(): string {
  const extras = selectedItemExtras();
  if (!extras.length) {
    return '';
  }

  return `
    <section class="panel page-panel item-section item-extras-section" data-nav-section="extras" data-nav-label="Extras">
      <div class="section-heading item-row-heading">
        <h3>Extras <span class="item-row-count">· ${extras.length}</span></h3>
      </div>
      <div class="shelf-row-shell item-row-shell">
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="item-extras:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
        <div class="shelf-row extras-row item-responsive-row" data-item-row="extras" data-shelf-row="item-extras">
          ${extras.map((extra, index) => renderMediaExtraCard(extra, index)).join('')}
        </div>
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="item-extras:1" title="Scroll right">${renderIcon('chevron-right')}</button>
      </div>
    </section>
  `;
}

export function renderSelectedItemCollectionRails(): string {
  const rails = selectedItemCollectionRails();
  if (!rails.length) {
    return '';
  }

  return rails
    .map((rail, index) => {
      const rowId = `item-collection-${index}`;
      return `
        <section class="panel page-panel item-section item-collection-section" data-nav-section="collection:${escapeHtml(rail.collection.id)}" data-nav-label="${escapeHtml(rail.collection.name)}">
          <div class="section-heading section-heading-actions">
            <div>
              <h3>${escapeHtml(rail.collection.name)}</h3>
              <p class="muted">Also in this collection</p>
            </div>
            <button type="button" class="secondary-button" data-collection-filter="${escapeHtml(rail.collection.id)}">${renderButtonContent('View collection', 'arrow-right')}</button>
          </div>
          <div class="shelf-row-shell">
            <button type="button" class="shelf-scroll-button" data-shelf-scroll="${rowId}:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
            <div class="shelf-row" data-shelf-row="${rowId}">${rail.items.map(renderItemCard).join('')}</div>
            <button type="button" class="shelf-scroll-button" data-shelf-scroll="${rowId}:1" title="Scroll right">${renderIcon('chevron-right')}</button>
          </div>
        </section>
      `;
    })
    .join('');
}

export function itemSortKey(item: MediaItemSummary): string {
  const season = typeof item.season_number === 'number' ? String(item.season_number).padStart(5, '0') : '99999';
  const episode = typeof item.episode_number === 'number' ? String(item.episode_number).padStart(5, '0') : '99999';
  return `${season}:${episode}:${item.display_title.toLocaleLowerCase()}`;
}

export function compareMediaItems(left: MediaItemSummary, right: MediaItemSummary): number {
  return itemSortKey(left).localeCompare(itemSortKey(right));
}

export function personCreditRootItem(entry: MetadataPersonItemCredit): MediaItemSummary {
  return entry.hierarchy.find((item) => item.item_type === 'show')
    ?? entry.hierarchy[0]
    ?? entry.item;
}

export function personCreditSeasonItem(entry: MetadataPersonItemCredit): MediaItemSummary | undefined {
  if (entry.item.item_type === 'season') {
    return entry.item;
  }

  if (entry.item.item_type !== 'episode') {
    return undefined;
  }

  return [...entry.hierarchy].reverse().find((item) => item.item_type === 'season');
}

export function personCreditGroups(credits: MetadataPersonItemCredit[]): PersonCreditGroup[] {
  const groupsByRootId = new Map<number, PersonCreditGroup>();

  credits.forEach((entry) => {
    const root = personCreditRootItem(entry);
    if (!groupsByRootId.has(root.id)) {
      groupsByRootId.set(root.id, { root, seasons: [] });
    }

    const group = groupsByRootId.get(root.id)!;
    const season = personCreditSeasonItem(entry);
    if (!season) {
      return;
    }

    let seasonGroup = group.seasons.find((candidate) => candidate.season.id === season.id);
    if (!seasonGroup) {
      seasonGroup = { season, episodes: [] };
      group.seasons.push(seasonGroup);
    }

    if (entry.item.item_type === 'episode' && !seasonGroup.episodes.some((episode) => episode.id === entry.item.id)) {
      seasonGroup.episodes.push(entry.item);
    }
  });

  const groups = [...groupsByRootId.values()]
    .map((group) => ({
      ...group,
      seasons: sortedPersonCreditSeasons(group),
    }));
  groups.sort((left, right) => left.root.display_title.localeCompare(right.root.display_title));
  return groups;
}

function sortedPersonCreditSeasons(group: PersonCreditGroup): PersonCreditGroup['seasons'] {
  const seasons = group.seasons.map((seasonGroup) => {
    const episodes = [...seasonGroup.episodes];
    episodes.sort(compareMediaItems);
    return {
      ...seasonGroup,
      episodes,
    };
  });
  seasons.sort((left, right) => compareMediaItems(left.season, right.season));
  return seasons;
}

function countLabel(count: number, singular: string): string {
  if (count <= 0) {
    return '';
  }
  return `${count} ${singular}${count === 1 ? '' : 's'}`;
}

export function renderPersonCreditGroup(group: PersonCreditGroup): string {
  const seasonCount = group.seasons.length;
  const episodeCount = group.seasons.reduce((total, season) => total + season.episodes.length, 0);
  const traySummary = [
    countLabel(seasonCount, 'season'),
    countLabel(episodeCount, 'episode'),
  ].filter(Boolean).join(' · ');
  const seasonTrayMarkup = group.seasons.length ? renderPersonSeasonCreditTray(group, traySummary) : '';

  return `
    <article class="person-credit-card" data-person-credit-card data-person-credit-id="${group.root.id}">
      ${renderItemCard(group.root)}
      ${seasonCount ? `<button type="button" class="secondary-button credit-expand-button" data-expand-credit="${group.root.id}" aria-expanded="false" aria-controls="credit-seasons-${group.root.id}">${renderButtonContent('View seasons', 'chevron-down')}</button>` : ''}
    </article>
    ${seasonTrayMarkup}
  `;
}

function renderPersonSeasonCreditTray(group: PersonCreditGroup, traySummary: string): string {
  return `
    <div class="person-credit-tray person-season-tray" id="credit-seasons-${group.root.id}" data-person-credit-tray data-person-credit-id="${group.root.id}" data-nav-section="credit-seasons:${group.root.id}" data-nav-label="Seasons">
      <div class="person-credit-tray-heading">
        <span>${escapeHtml(traySummary || 'Credits')}</span>
        <button class="person-credit-tray-close" type="button" data-close-person-credit-tray title="Collapse row" aria-label="Collapse row">${renderIcon('x')}</button>
      </div>
      <div class="person-season-credit-grid">
        ${group.seasons.map((seasonGroup) => {
          const episodeTrayMarkup = seasonGroup.episodes.length ? renderPersonEpisodeCreditTray(seasonGroup) : '';
          return `
            <article class="person-season-credit-card" data-person-season-credit-card data-person-season-credit-id="${seasonGroup.season.id}">
              ${renderItemCard(seasonGroup.season)}
              ${seasonGroup.episodes.length ? `<button type="button" class="secondary-button credit-expand-button" data-expand-season-credit="${seasonGroup.season.id}" aria-expanded="false" aria-controls="credit-episodes-${seasonGroup.season.id}">${renderButtonContent('View episodes', 'chevron-down')}</button>` : ''}
            </article>
            ${episodeTrayMarkup}
          `;
        }).join('')}
      </div>
    </div>
  `;
}

function renderPersonEpisodeCreditTray(seasonGroup: PersonCreditGroup['seasons'][number]): string {
  return `
    <div class="person-credit-tray person-episode-tray" id="credit-episodes-${seasonGroup.season.id}" data-person-season-credit-tray data-person-season-credit-id="${seasonGroup.season.id}" data-nav-section="credit-episodes:${seasonGroup.season.id}" data-nav-label="Episodes">
      <div class="person-credit-tray-heading">
        <span>${escapeHtml(countLabel(seasonGroup.episodes.length, 'episode'))}</span>
        <button class="person-credit-tray-close" type="button" data-close-person-season-credit-tray title="Collapse row" aria-label="Collapse row">${renderIcon('x')}</button>
      </div>
      <div class="person-episode-credit-grid">
        ${seasonGroup.episodes.map(renderItemCard).join('')}
      </div>
    </div>
  `;
}

export function renderPersonPage(): string {
  const response = state.selectedPerson;
  if (!response) {
    if (!state.isLoading && state.error) return '<div class="empty-state">Person details could not be loaded. Use Retry to try again.</div>';
    return renderLoadingDetail('Loading person details');
  }

  const personImageUrl = personPortraitUrl(response.person);
  const credits = response.credits;
  const creditGroups = personCreditGroups(credits);
  const age = personAgeLabel(response.person.birthday, response.person.deathday);
  const knownForTags = response.person.known_for
    .map((title) => `<span class="tag">${escapeHtml(title)}</span>`)
    .join('');
  const knownForMarkup = response.person.known_for.length ? `<div class="hero-meta-row">${knownForTags}</div>` : '';

  return `
    <section class="item-page person-page">
      <section class="item-hero person-hero" data-nav-section="person-overview" data-nav-label="About ${escapeHtml(response.person.name)}">
        <div class="detail-art item-poster person-poster ${personImageUrl ? 'has-image' : ''}">
          ${personImageUrl ? `<img src="${escapeHtml(personImageUrl)}" alt="${escapeHtml(response.person.name)}" />` : `<span>${escapeHtml(response.person.name.slice(0, 1).toUpperCase())}</span>`}
        </div>
        <div class="detail-summary item-summary">
          <h2 class="item-title-fallback">${escapeHtml(response.person.name)}</h2>
          <div class="hero-meta-row">
            <span class="tag">${escapeHtml(providerDisplayName(response.person.provider_id))}</span>
            <span class="tag">${credits.length} item${credits.length === 1 ? '' : 's'}</span>
            ${response.person.birthday ? `<span class="tag">${escapeHtml([formatPersonDate(response.person.birthday), age].filter(Boolean).join(' · '))}</span>` : ''}
            ${response.person.gender ? `<span class="tag">${escapeHtml(response.person.gender)}</span>` : ''}
          </div>
          ${response.person.birth_place ? `<p class="hero-tagline">${escapeHtml(response.person.birth_place)}</p>` : ''}
          ${response.person.biography ? renderCollapsibleText(response.person.biography, `person-biography:${response.person.id}`) : ''}
          ${knownForMarkup}
          <div class="detail-actions">
            <button type="button" class="secondary-button" id="back-to-library">${renderButtonContent('Back', 'arrow-left')}</button>
            ${response.person.profile_url ? `<a class="button-link secondary-button" href="${escapeHtml(response.person.profile_url)}" target="_blank" rel="noreferrer">Provider page</a>` : ''}
          </div>
        </div>
      </section>

      <section class="panel page-panel item-section" data-nav-section="credits" data-nav-label="Credits">
        <div class="section-heading section-heading-actions">
          <h3>Credits</h3>
          <span class="muted">${creditGroups.length} title${creditGroups.length === 1 ? '' : 's'}</span>
        </div>
        ${credits.length
          ? `<div class="person-credit-grid">${creditGroups.map(renderPersonCreditGroup).join('')}</div>`
          : '<div class="empty-state tight">No linked items are stored for this person yet.</div>'}
      </section>
    </section>
  `;
}

function personPortraitUrl(person: NonNullable<typeof state.selectedPerson>['person']): string | undefined {
  if (person.cached_image_path) return getPersonImageUrl(person.id);
  return person.image_url ? resolveApiUrl(person.image_url) : undefined;
}

export function directGridChildren(grid: HTMLElement, selector: string): HTMLElement[] {
  return Array.from(grid.children)
    .filter((child): child is HTMLElement => child instanceof HTMLElement && child.matches(selector));
}

export function directGridChildByData(grid: HTMLElement, selector: string, key: string, value: string | undefined): HTMLElement | undefined {
  if (!value) {
    return undefined;
  }

  return directGridChildren(grid, selector).find((child) => child.dataset[key] === value);
}

export function rowIndexForElement(element: HTMLElement, rowTops: number[]): number {
  const rowIndex = rowTops.findIndex((top) => Math.abs(top - element.offsetTop) < 8);
  return Math.max(rowIndex, 0);
}

export function activatePersonCreditTray(
  grid: HTMLElement,
  card: HTMLElement,
  tray: HTMLElement,
  cardSelector: string,
  traySelector: string,
): void {
  if (card.classList.contains('is-active') && tray.classList.contains('is-active')) {
    return;
  }

  const cards = directGridChildren(grid, cardSelector);
  const trays = directGridChildren(grid, traySelector);

  trays.forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });
  cards.forEach((entry) => {
    entry.classList.remove('is-active');
    entry.querySelector('[aria-expanded]')?.setAttribute('aria-expanded', 'false');
    entry.style.removeProperty('order');
  });

  const rowTops = [...new Set(cards
    .map((entry) => entry.offsetTop)
    .sort((left, right) => left - right)
    .filter((top, index, values) => index === 0 || Math.abs(top - values[index - 1]) >= 8))];

  cards.forEach((entry) => {
    entry.style.order = String(rowIndexForElement(entry, rowTops) * 2);
  });

  card.classList.add('is-active');
  card.querySelector('[aria-expanded]')?.setAttribute('aria-expanded', 'true');
  tray.classList.add('is-active');
  tray.style.order = String(rowIndexForElement(card, rowTops) * 2 + 1);
}

export function collapsePersonCreditTrays(grid: HTMLElement, cardSelector: string, traySelector: string): void {
  directGridChildren(grid, cardSelector).forEach((entry) => {
    entry.classList.remove('is-active');
    entry.querySelector('[aria-expanded]')?.setAttribute('aria-expanded', 'false');
    entry.style.removeProperty('order');
  });
  directGridChildren(grid, traySelector).forEach((entry) => {
    entry.classList.remove('is-active');
    entry.style.removeProperty('order');
  });
}

export function bindPersonCreditTrays(): void {
  const grid = document.querySelector<HTMLElement>('.person-credit-grid');
  if (!grid) {
    return;
  }

  const activateRootTray = (target: EventTarget | null): void => {
    const card = target instanceof Element ? target.closest<HTMLElement>('.person-credit-card') : null;
    if (card?.parentElement !== grid) {
      return;
    }

    const tray = directGridChildByData(grid, '.person-season-tray', 'personCreditId', card.dataset.personCreditId);
    if (!tray) {
      return;
    }

    activatePersonCreditTray(grid, card, tray, '.person-credit-card', '.person-season-tray');
  };

  const activateSeasonTray = (target: EventTarget | null): void => {
    const card = target instanceof Element ? target.closest<HTMLElement>('.person-season-credit-card') : null;
    const seasonGrid = card?.parentElement;
    if (!card || !(seasonGrid instanceof HTMLElement) || !seasonGrid.classList.contains('person-season-credit-grid')) {
      return;
    }

    const tray = directGridChildByData(seasonGrid, '.person-episode-tray', 'personSeasonCreditId', card.dataset.personSeasonCreditId);
    if (!tray) {
      return;
    }

    activatePersonCreditTray(seasonGrid, card, tray, '.person-season-credit-card', '.person-episode-tray');
  };

  grid.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const rootExpand = target?.closest<HTMLElement>('[data-expand-credit]');
    const seasonExpand = target?.closest<HTMLElement>('[data-expand-season-credit]');
    if (rootExpand) {
      if (rootExpand.getAttribute('aria-expanded') === 'true') {
        collapsePersonCreditTrays(grid, '.person-credit-card', '.person-season-tray');
        state.expandedCreditId = undefined;
      } else {
        activateRootTray(rootExpand);
        state.expandedCreditId = Number(rootExpand.dataset.expandCredit);
      }
      state.expandedSeasonCreditId = undefined;
    }
    if (seasonExpand) {
      const seasonGrid = seasonExpand.closest<HTMLElement>('.person-season-credit-grid');
      if (seasonGrid && seasonExpand.getAttribute('aria-expanded') === 'true') {
        collapsePersonCreditTrays(seasonGrid, '.person-season-credit-card', '.person-episode-tray');
        state.expandedSeasonCreditId = undefined;
      } else {
        activateSeasonTray(seasonExpand);
        state.expandedSeasonCreditId = Number(seasonExpand.dataset.expandSeasonCredit);
      }
    }
    const rootCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-credit-tray]');
    if (rootCloseButton) {
      event.preventDefault();
      event.stopPropagation();
      grid.querySelector<HTMLElement>('[data-expand-credit][aria-expanded="true"]')?.focus({ preventScroll: true });
      collapsePersonCreditTrays(grid, '.person-credit-card', '.person-season-tray');
      state.expandedCreditId = undefined;
      state.expandedSeasonCreditId = undefined;
      return;
    }

    const seasonCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-season-credit-tray]');
    const seasonGrid = seasonCloseButton?.closest<HTMLElement>('.person-season-credit-grid');
    if (seasonCloseButton && seasonGrid) {
      event.preventDefault();
      event.stopPropagation();
      seasonGrid.querySelector<HTMLElement>('[data-expand-season-credit][aria-expanded="true"]')?.focus({ preventScroll: true });
      collapsePersonCreditTrays(seasonGrid, '.person-season-credit-card', '.person-episode-tray');
      state.expandedSeasonCreditId = undefined;
    }
  });
  if (state.expandedCreditId) activateRootTray(grid.querySelector(`[data-expand-credit="${state.expandedCreditId}"]`));
  if (state.expandedSeasonCreditId) activateSeasonTray(grid.querySelector(`[data-expand-season-credit="${state.expandedSeasonCreditId}"]`));
}

function selectedItemOverview(item: MediaItemDetail): string {
  return item.overview
    ?? state.selectedItemMetadata?.matches[0]?.overview
    ?? 'No description is stored for this item yet.';
}

function selectedItemChildSectionTitle(item: MediaItemDetail): string {
  if (item.item_type === 'show') {
    return 'Seasons';
  }

  if (item.item_type === 'artist') return 'Albums';
  if (item.item_type === 'album') return 'Tracks';
  return item.item_type === 'season' ? 'Episodes' : 'Contained items';
}

function renderSelectedItemBreadcrumbs(item: MediaItemDetail): string {
  if (!item.hierarchy.length) return '';
  return `
        <nav class="item-breadcrumbs panel page-panel" aria-label="Item hierarchy" data-nav-section="hierarchy" data-nav-label="Item hierarchy">
          ${item.hierarchy.map((hierarchyItem) => `
            <button type="button" class="breadcrumb-button" data-item-id="${hierarchyItem.id}">${escapeHtml(hierarchyItem.display_title)}</button>
          `).join('<span class="breadcrumb-separator">/</span>')}
        </nav>
      `;
}

function renderSelectedItemPoster(item: MediaItemDetail, posterUrl: string | undefined): string {
  return posterUrl
    ? `<img data-item-cover-image src="${escapeHtml(posterUrl)}" alt="${escapeHtml(item.display_title)} poster" /><span class="item-cover-fallback" hidden aria-hidden="true">${escapeHtml(item.display_title.slice(0, 1).toUpperCase())}</span>`
    : `<span>${escapeHtml(item.display_title.slice(0, 1).toUpperCase())}</span>`;
}

/** Keep the current episode and remaining time beside the primary playback action. */
function selectedItemRemainingTime(item: MediaItemDetail): string | undefined {
  const playbackTarget = item.playback_target;
  const playable = item.playable ? item : state.libraryItems.find((entry) => entry.id === playbackTarget?.item_id)
    ?? item.children.find((entry) => entry.id === playbackTarget?.item_id);
  const duration = playable?.playback_duration_ms ?? playable?.duration_ms ?? 0;
  const position = item.playable ? resumablePlaybackPositionMs(item) : playbackTarget?.start_ms ?? 0;
  return position > 0 && duration > position ? `${formatDuration(duration - position)} remaining` : undefined;
}

function renderSelectedItemHeroMeta(item: MediaItemDetail, genres: string[]): string {
  const tags = [missingItemDetailBadgeMarkup(item)];
  if (item.release_year) {
    tags.push(`<span class="tag">${item.release_year}</span>`);
  }
  if (item.content_rating) {
    tags.push(`<span class="tag">${escapeHtml(item.content_rating)}</span>`);
  }
  tags.push(renderRating(item));
  if (item.playable && item.duration_ms) tags.push(`<span class="tag">${escapeHtml(formatDuration(item.duration_ms))}</span>`);
  const genreTags = genres.map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`).join('');
  return `<div class="hero-meta-row">${tags.join('')}${genreTags}</div>`;
}

function renderResumeButton(item: MediaItemDetail, resumeMs: number): string {
  if (!item.playable || resumeMs <= 0) {
    return '';
  }

  const resumeLabel = `Resume ${formatDuration(resumeMs)}`;
  return `<button type="button" data-resume-hold aria-describedby="resume-hold-hint" aria-controls="resume-playback-menu" aria-expanded="${state.resumeOptionsOpen}" data-play-selected-item-start-ms="${resumeMs}">${renderButtonContent(resumeLabel, 'play', 'start', selectedItemRemainingTime(item))}</button>`;
}

function renderPrimaryPlayButton(item: MediaItemDetail, resumeMs: number, id?: string, restart = false): string {
  if (!item.playable) {
    return '';
  }

  const isRestart = restart || resumeMs > 0;
  const playButtonClass = isRestart ? 'secondary-button' : '';
  let playButtonLabel = 'Play';
  if (isRestart) playButtonLabel = item.item_type === 'episode' ? 'Restart episode' : 'Start over';
  const idAttribute = id ? `id="${escapeHtml(id)}"` : '';
  return `<button type="button" ${idAttribute} class="${playButtonClass}" data-play-selected-item-start-ms="0">${renderButtonContent(playButtonLabel, isRestart ? 'rotate-ccw' : 'play')}</button>`;
}

function renderTrailerActionButton(preferredTrailer: TrailerOption | undefined, trailerButtonTitle: string): string {
  return preferredTrailer
    ? `<button type="button" class="secondary-button" id="play-item-trailer" aria-label="Play trailer" title="${escapeHtml(trailerButtonTitle)}">${renderButtonContent('Trailer', 'clapperboard')}</button>`
    : '';
}

function renderThemeSongButton(themeSongOption: ReturnType<typeof currentThemeSongYouTubeTarget>): string {
  return themeSongOption
    ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'music')}</button>`
    : '';
}

interface SelectedItemActionsOptions {
  item: MediaItemDetail;
  resumeMs: number;
  playbackTarget?: MediaPlaybackTarget;
  restartPlaybackTarget?: MediaPlaybackTarget;
  preferredTrailer?: TrailerOption;
  themeSongOption?: ReturnType<typeof currentThemeSongYouTubeTarget>;
  trailerButtonTitle: string;
  hasMultipleTrailers: boolean;
}

function renderResumeOptions(item: MediaItemDetail, resumeMs: number, playbackTarget?: MediaPlaybackTarget): string {
  const canResume = resumeMs > 0 || (playbackTarget?.start_ms ?? 0) > 0;
  if (!canResume) return '';
  const restartButton = item.playable
    ? renderPrimaryPlayButton(item, resumeMs, 'resume-start-over')
    : `<button type="button" id="resume-start-over" class="secondary-button" data-playback-target-item-id="${playbackTarget!.item_id}" data-playback-target-start-ms="0">${renderButtonContent('Restart episode', 'rotate-ccw')}</button>`;
  return `<span class="sr-only" id="resume-hold-hint" aria-live="polite">Hold for playback options</span>
    <section class="resume-playback-menu" id="resume-playback-menu" aria-label="Playback options" data-nav-section="resume-options" data-nav-label="Playback options" ${state.resumeOptionsOpen ? '' : 'hidden'}>
      ${restartButton}
    </section>`;
}

function renderSelectedItemActions(options: SelectedItemActionsOptions): string {
  const {
    item,
    resumeMs,
    playbackTarget,
    restartPlaybackTarget,
    preferredTrailer,
    themeSongOption,
    trailerButtonTitle,
    hasMultipleTrailers,
  } = options;
  const tvTargets = tvRestartTargets(item);
  const targetSubtitle = playbackTarget ? [playbackTarget.display_title, selectedItemRemainingTime(item)].filter(Boolean).join(' · ') : undefined;

  return `
          <div class="detail-actions" id="item-playback-actions">
            <div class="item-primary-action">
            <div class="item-bar-identity" data-sticky-identity hidden>
              ${renderWatchProgress(item)}<strong>${escapeHtml(item.display_title)}</strong>
            </div>
            ${item.item_type === 'album' && item.children.some((track) => track.playable && !track.missing_since) ? `<button type="button" class="primary-button" data-play-music-album-id="${item.id}">${renderButtonContent('Play album', 'play')}</button>` : ''}
            ${renderResumeButton(item, resumeMs)}
            ${resumeMs > 0 ? '' : renderPrimaryPlayButton(item, resumeMs)}
            ${playbackTarget ? renderPlaybackTargetButton({ ...playbackTarget, label: tvPlaybackLabel(item, playbackTarget) }, false, { holdForOptions: playbackTarget.start_ms > 0, subtitle: targetSubtitle }) : ''}
            ${renderResumeOptions(item, resumeMs, playbackTarget)}
            </div>
            ${renderTrailerActionButton(preferredTrailer, trailerButtonTitle)}
            <details class="item-more" id="item-more" ${state.itemMoreOpen ? 'open' : ''}>
              <summary class="secondary-button" aria-label="More actions">${renderButtonContent('More', 'ellipsis')}</summary>
              <div class="item-more-menu">
                ${resumeMs > 0 || item.item_type === 'episode' ? renderPrimaryPlayButton(item, resumeMs, 'item-start-over', true) : ''}
                ${tvTargets.map((target) => renderPlaybackTargetButton(target, true, { id: target.label === 'Restart show' ? 'restart-show' : 'restart-season' })).join('')}
                ${!tvTargets.length && restartPlaybackTarget ? renderPlaybackTargetButton(restartPlaybackTarget, true, { id: 'item-start-over' }) : ''}
                ${hasMultipleTrailers ? `<button type="button" class="secondary-button" id="choose-item-trailer">${renderButtonContent('Choose a trailer', 'clapperboard')}</button>` : ''}
                ${renderThemeSongButton(themeSongOption)}
                <button type="button" class="secondary-button" id="show-item-details">${renderButtonContent(canManageUsers() ? 'Details and management' : 'Media details', 'settings')}</button>
              </div>
            </details>
            ${renderEpisodeNavigation(item)}
            <button type="button" class="secondary-button item-back-button" id="back-to-library">${renderButtonContent(backNavigationTarget().label, 'arrow-left')}</button>
          </div>
  `;
}

function playableTarget(item: MediaItemDetail): MediaPlaybackTarget | undefined {
  return item.playable ? undefined : item.playback_target ?? undefined;
}

function restartPlayableTarget(item: MediaItemDetail): MediaPlaybackTarget | undefined {
  return item.playable ? undefined : item.restart_playback_target ?? undefined;
}

function renderTrailerPicker(trailerOptions: TrailerOption[], hasMultipleTrailers: boolean): string {
  if (!hasMultipleTrailers || !state.isTrailerMenuOpen) {
    return '';
  }

  return `
            <section class="trailer-picker panel" data-nav-section="trailers" data-nav-label="Trailers">
              <div class="section-heading section-heading-actions">
                <h3>Choose a trailer</h3>
                <button type="button" class="secondary-button" id="close-trailer-picker">${renderButtonContent('Close', 'x')}</button>
              </div>
              <div class="trailer-picker-list">
                ${trailerOptions.map((option, index) => `
                  <button type="button" class="secondary-button trailer-option-button" data-play-trailer-index="${index}">${escapeHtml(option.title)}</button>
                `).join('')}
              </div>
            </section>
          `;
}

function selectedItemTechnicalFacts(item: MediaItemDetail): Array<{ label: string; value: string }> {
  return [
    { label: 'Duration', value: formatDuration(item.duration_ms) },
    {
      label: 'Format',
      value: [item.container?.toUpperCase(), item.media_kind.toUpperCase()].filter(Boolean).join(' • ') || 'Unknown',
    },
    {
      label: 'Codecs',
      value: [item.video_codec, item.audio_codec].filter(Boolean).join(' / ') || 'Unknown',
    },
    {
      label: 'Resolution',
      value: item.width && item.height ? `${item.width}×${item.height}` : 'Unknown',
    },
    { label: 'Bitrate', value: formatBitRate(item.bit_rate) },
    { label: 'Size', value: formatFileSize(item.file_size) },
  ];
}

function renderSelectedItemFactList(item: MediaItemDetail): string {
  return `
          <div class="item-fact-list">
            ${selectedItemTechnicalFacts(item).map((fact) => `
              <div class="item-fact">
                <span class="label">${escapeHtml(fact.label)}</span>
                <strong>${escapeHtml(fact.value)}</strong>
              </div>
            `).join('')}
          </div>
  `;
}

function renderSelectedItemIdentity(
  item: MediaItemDetail,
  posterUrl: string | undefined,
  logoUrl: string | undefined,
  genres: string[],
): string {
  return `
      <aside class="item-cover-column" aria-label="Cover artwork">
        <div class="detail-art item-poster ${posterUrl ? 'has-image' : ''}">
          ${renderSelectedItemPoster(item, posterUrl)}
          ${renderWatchProgress(item)}
          ${renderMetadataRefreshBadge(item)}
        </div>
      </aside>
      <header class="item-identity">
          ${renderSelectedItemBreadcrumbs(item)}
          ${renderTvTitle(item, logoUrl)}
          ${item.tagline ? `<p class="hero-tagline">${escapeHtml(item.tagline)}</p>` : ''}
          ${renderSelectedItemHeroMeta(item, genres)}
      </header>
  `;
}

export function renderEpisodeCard(episode: MediaItemSummary): string {
  return renderTvEpisodeCard(episode);
}

/** Keep season navigation separate from the current season's episode list. */
export function renderEpisodeBrowser(item: MediaItemDetail): string {
  return renderTvChildren(item);
}

function renderSelectedItemChildrenSection(item: MediaItemDetail): string {
  if (item.item_type === 'album') return renderAlbumTracks(item);
  if (['show', 'season', 'episode'].includes(item.item_type)) return renderTvChildren(item);

  if (!item.children.length) {
    return '';
  }

  const children = item.children;
  const childCountLabel = countLabel(children.length, 'item');
  return `
        <section class="panel page-panel item-section" data-nav-section="children" data-nav-label="${escapeHtml(selectedItemChildSectionTitle(item))}">
          <div class="section-heading section-heading-actions">
            <h3>${escapeHtml(selectedItemChildSectionTitle(item))}</h3>
            <span class="muted">${childCountLabel}</span>
          </div>
          <div class="item-grid hierarchy-item-grid">${children.map(renderItemCard).join('')}</div>
        </section>
      `;
}

function renderMetadataRefreshButton(
  supportsManualLinking: boolean,
  linkedMatch: ItemMetadataMatch | undefined,
  metadataRefreshActive: boolean,
): string {
  if (!supportsManualLinking) {
    return '';
  }

  const refreshButtonDisabled = linkedMatch && !metadataRefreshActive ? '' : 'disabled';
  const refreshButtonLabel = metadataRefreshActive ? 'Refreshing metadata' : 'Force refresh metadata';
  return `<div class="metadata-management-actions">
    <button type="button" class="secondary-button" id="refresh-item-metadata" ${refreshButtonDisabled}>${renderButtonContent(refreshButtonLabel, 'refresh-cw')}</button>
    ${linkedMatch ? `<button type="button" class="secondary-button" id="unlink-item-metadata" title="Clear linked metadata now. For shows, this also clears seasons and episodes.">${renderButtonContent('Unlink metadata', 'x')}</button>` : ''}
  </div>`;
}

function renderMetadataSearchPanel(supportsManualLinking: boolean): string {
  if (!supportsManualLinking) {
    return '<div class="empty-state tight">Season and episode metadata is inherited and refreshed automatically from the linked show.</div>';
  }

  return `
              <form id="metadata-search-form" class="metadata-search-form">
                <input id="metadata-search-input" name="metadataSearch" type="search" value="${escapeHtml(state.metadataSearchQuery)}" placeholder="${escapeHtml(selectedItemDefaultMetadataTitle() || 'Title')}" autocomplete="off" />
                <input id="metadata-search-year" name="metadataSearchYear" type="number" min="1800" max="2200" value="${escapeHtml(state.metadataSearchYear)}" placeholder="${escapeHtml(selectedItemDefaultMetadataYear() || 'Year')}" autocomplete="off" />
                <input id="metadata-search-language" name="metadataSearchLanguage" type="text" value="${escapeHtml(state.metadataSearchLanguage)}" placeholder="${escapeHtml(defaultMetadataSearchLanguage())}" autocomplete="off" />
                ${renderMetadataSearchProviderControls()}
                <button type="submit">${renderButtonContent('Search metadata', 'search')}</button>
              </form>
              <div class="metadata-search-list">${renderMetadataSearchResults()}</div>
            `;
}

function renderSelectedItemSupportGrid(
  item: MediaItemDetail,
  library: ReturnType<typeof activeLibrary>,
  supportsManualLinking: boolean,
  metadataRefreshButtonMarkup: string,
  metadataSearchPanel: string,
): string {
  const metadataHeading = supportsManualLinking ? 'Link metadata' : 'Metadata';
  return `
      <details class="item-management" id="item-management" ${state.itemManagementOpen ? 'open' : ''} data-nav-section="management" data-nav-label="Media details">
        <summary>${renderButtonContent(canManageUsers() ? 'Details and management' : 'Media details', 'settings')}${renderIcon('chevron-down', 'disclosure-chevron')}</summary>
        <div class="item-management-body">
        ${renderSelectedItemFactList(item)}
        ${state.selectedPlayback?.reason ? `<p class="muted playback-detail-reason">${escapeHtml(state.selectedPlayback.reason)}</p>` : ''}
      <div class="item-support-grid">
        <section class="panel page-panel item-section">
          <div class="section-heading">
            <h3>File and library</h3>
          </div>
          <div class="item-info-list">
            <div>
              <span class="label">Library</span>
              <strong>${escapeHtml(library?.name ?? 'Unknown')}</strong>
            </div>
            <div>
              <span class="label">Folders</span>
              <strong>${escapeHtml(String(library?.paths.length ?? 0))}</strong>
            </div>
            <div>
              <span class="label">Source</span>
              <strong>${escapeHtml(item.relative_path)}</strong>
            </div>
            <div>
              <span class="label">Updated</span>
              <strong>${escapeHtml(formatTimestamp(item.modified_at))}</strong>
            </div>
          </div>
        </section>

        ${canManageUsers() ? `<section class="panel page-panel item-section item-link-panel" data-nav-section="metadata" data-nav-label="Metadata">
          <div class="section-heading section-heading-actions">
            <h3>${metadataHeading}</h3>
            ${metadataRefreshButtonMarkup}
          </div>
          ${renderLinkedMetadataSummary()}
          ${metadataSearchPanel}
        </section>` : ''}
      </div>
      </div>
      </details>
  `;
}

function renderSelectedItemPage(item: MediaItemDetail): string {
  const artwork = itemArtwork(item);
  const trailerOptions = currentTrailerOptions();
  const hasMultipleTrailers = trailerOptions.length > 1;
  const supportsManualLinking = canManuallyLinkMetadata(item);
  const linkedMatch = state.selectedItemMetadata?.matches[0];
  const metadataRefreshActive = itemHasActiveMetadataRefresh(item);
  const resumeMs = resumablePlaybackPositionMs(item);
  const trailerButtonTitle = hasMultipleTrailers
    ? 'Click to play the first trailer. Right-click or press and hold to choose another trailer.'
    : 'Play Trailer';
  const actionsMarkup = renderSelectedItemActions({
    item,
    resumeMs,
    playbackTarget: playableTarget(item),
    restartPlaybackTarget: restartPlayableTarget(item),
    preferredTrailer: trailerOptions[0],
    themeSongOption: currentThemeSongYouTubeTarget(),
    trailerButtonTitle,
    hasMultipleTrailers,
  });
  const metadataRefreshButtonMarkup = renderMetadataRefreshButton(supportsManualLinking, linkedMatch, metadataRefreshActive);

  return `
    <section class="item-page item-detail-page ${item.item_type === 'episode' ? 'episode-detail-page' : ''} ${item.music ? 'music-detail-page' : ''}" data-item-page-id="${item.id}">
      ${artwork.backdrop ? `<div class="item-opening-backdrop" aria-hidden="true" style="--page-backdrop-image: url('${escapeHtml(artwork.backdrop)}');"></div>` : ''}
      ${renderSelectedItemIdentity(
        item,
        artwork.poster,
        artwork.logo,
        item.genres.length ? item.genres : showForItem(item)?.genres ?? [],
      )}
      <span class="item-actions-anchor" aria-hidden="true"></span>
      <div class="item-action-dock" data-nav-section="item-overview" data-nav-label="Playback actions">
        ${actionsMarkup}
        ${renderSeasonTabs(item)}
      </div>
      <section class="item-description-section" data-nav-section="description" data-nav-label="Description">
        ${renderCollapsibleText(selectedItemOverview(item), `item-overview:${item.id}`, 'hero-description', true)}
      </section>
      <div class="item-support-content" id="item-support-content">
      ${renderTrailerPicker(trailerOptions, hasMultipleTrailers)}
      ${renderSelectedItemChildrenSection(item)}

      ${renderItemExtrasRail()}

      ${renderPeopleRail()}

      ${renderSelectedItemCollectionRails()}

      ${renderSelectedItemSupportGrid(
        item,
        state.libraries.find((entry) => entry.id === item.library_id),
        supportsManualLinking,
        metadataRefreshButtonMarkup,
        renderMetadataSearchPanel(supportsManualLinking),
      )}
      </div>
    </section>
  `;
}

export function renderItemPage(): string {
  if (!state.selectedItem) {
    if (!state.isLoading && state.error) return '<div class="empty-state">Item details could not be loaded. Use Retry to try again.</div>';
    return renderLoadingDetail('Loading item details', true);
  }

  return renderSelectedItemPage(state.selectedItem);
}
