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
import { getArtworkUrl, getPersonImageUrl, resolveApiUrl } from '../api';
import { escapeHtml, formatBitRate, formatDuration, formatFileSize, formatTimestamp } from './format';
import { normalizedMetadataLanguages } from './formUtils';
import { extractYouTubeVideoId } from './youtube';
import { mediaExtraDurationLabel, mediaExtraTitle, mediaExtraTypeLabel } from './mediaExtras';
import { currentThemeSongYouTubeTarget, currentTrailerOptions } from './mediaTargets';
import { itemHasActiveMetadataRefresh, itemIsMetadataPending } from './activities';
import { resumablePlaybackPositionMs } from './playbackProgress';
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
import { missingItemDetailBadgeMarkup, playbackDetailBadgeMarkup, renderItemCard, renderPlaybackTargetButton } from './homeView';

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

  return `
    <section class="panel page-panel item-section item-people-section">
      <div class="section-heading section-heading-actions">
        <h3>People</h3>
        <span class="muted">${people.length} credit${people.length === 1 ? '' : 's'}</span>
      </div>
      <div class="shelf-row-shell people-row-shell">
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="people:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
        <div class="people-row" data-shelf-row="people">
          ${people.map(renderPersonCredit).join('')}
        </div>
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="people:1" title="Scroll right">${renderIcon('chevron-right')}</button>
      </div>
    </section>
  `;
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
    <section class="panel page-panel item-section item-extras-section">
      <div class="section-heading section-heading-actions">
        <h3>Extras</h3>
        <span class="muted">${extras.length} video${extras.length === 1 ? '' : 's'}</span>
      </div>
      <div class="shelf-row-shell">
        <button type="button" class="shelf-scroll-button" data-shelf-scroll="item-extras:-1" title="Scroll left">${renderIcon('chevron-left')}</button>
        <div class="shelf-row extras-row" data-shelf-row="item-extras">
          ${extras.map(renderMediaExtraCard).join('')}
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
        <section class="panel page-panel item-section item-collection-section">
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

  return [...groupsByRootId.values()]
    .map((group) => ({
      ...group,
      seasons: group.seasons
        .map((seasonGroup) => ({
          ...seasonGroup,
          episodes: seasonGroup.episodes.sort(compareMediaItems),
        }))
        .sort((left, right) => compareMediaItems(left.season, right.season)),
    }))
    .sort((left, right) => left.root.display_title.localeCompare(right.root.display_title));
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
    </article>
    ${seasonTrayMarkup}
  `;
}

function renderPersonSeasonCreditTray(group: PersonCreditGroup, traySummary: string): string {
  return `
    <div class="person-credit-tray person-season-tray" data-person-credit-tray data-person-credit-id="${group.root.id}">
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
    <div class="person-credit-tray person-episode-tray" data-person-season-credit-tray data-person-season-credit-id="${seasonGroup.season.id}">
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
    return '<section class="panel page-panel"><div class="empty-state">Loading person details…</div></section>';
  }

  let personImageUrl: string | undefined;
  if (response.person.cached_image_path) {
    personImageUrl = getPersonImageUrl(response.person.id);
  } else if (response.person.image_url) {
    personImageUrl = resolveApiUrl(response.person.image_url);
  }
  const credits = response.credits;
  const creditGroups = personCreditGroups(credits);
  const age = personAgeLabel(response.person.birthday, response.person.deathday);
  const knownForTags = response.person.known_for
    .map((title) => `<span class="tag">${escapeHtml(title)}</span>`)
    .join('');
  const knownForMarkup = response.person.known_for.length ? `<div class="hero-meta-row">${knownForTags}</div>` : '';

  return `
    <section class="item-page person-page">
      <section class="item-hero person-hero">
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

      <section class="panel page-panel item-section">
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
  return rowIndex >= 0 ? rowIndex : 0;
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
  tray.classList.add('is-active');
  tray.style.order = String(rowIndexForElement(card, rowTops) * 2 + 1);
}

export function collapsePersonCreditTrays(grid: HTMLElement, cardSelector: string, traySelector: string): void {
  directGridChildren(grid, cardSelector).forEach((entry) => {
    entry.classList.remove('is-active');
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
    if (!card || card.parentElement !== grid) {
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

  grid.addEventListener('mouseover', (event) => {
    activateRootTray(event.target);
    activateSeasonTray(event.target);
  });
  grid.addEventListener('focusin', (event) => {
    activateRootTray(event.target);
    activateSeasonTray(event.target);
  });
  grid.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const rootCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-credit-tray]');
    if (rootCloseButton) {
      event.preventDefault();
      event.stopPropagation();
      collapsePersonCreditTrays(grid, '.person-credit-card', '.person-season-tray');
      return;
    }

    const seasonCloseButton = target?.closest<HTMLButtonElement>('[data-close-person-season-credit-tray]');
    const seasonGrid = seasonCloseButton?.closest<HTMLElement>('.person-season-credit-grid');
    if (seasonCloseButton && seasonGrid) {
      event.preventDefault();
      event.stopPropagation();
      collapsePersonCreditTrays(seasonGrid, '.person-season-credit-card', '.person-episode-tray');
    }
  });
}

function selectedItemPosterUrl(item: MediaItemDetail): string | undefined {
  return item.poster_url
    ? getArtworkUrl(item.id, 'poster', item.artwork_updated_at)
    : undefined;
}

function selectedItemLogoUrl(item: MediaItemDetail): string | undefined {
  return item.logo_url ? resolveApiUrl(item.logo_url) : undefined;
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

  return item.item_type === 'season' ? 'Episodes' : 'Contained items';
}

function renderSelectedItemBreadcrumbs(item: MediaItemDetail): string {
  if (!item.hierarchy.length) {
    return '';
  }

  return `
        <nav class="item-breadcrumbs panel page-panel" aria-label="Item hierarchy">
          ${item.hierarchy.map((hierarchyItem) => `
            <button type="button" class="breadcrumb-button" data-item-id="${hierarchyItem.id}">${escapeHtml(hierarchyItem.display_title)}</button>
          `).join('<span class="breadcrumb-separator">/</span>')}
          <span class="breadcrumb-separator">/</span>
          <span class="breadcrumb-current">${escapeHtml(item.display_title)}</span>
        </nav>
      `;
}

function renderSelectedItemPoster(item: MediaItemDetail, posterUrl: string | undefined): string {
  return posterUrl
    ? `<img src="${escapeHtml(posterUrl)}" alt="${escapeHtml(item.display_title)} poster" />`
    : `<span>${escapeHtml(item.display_title.slice(0, 1).toUpperCase())}</span>`;
}

function renderSelectedItemTitle(item: MediaItemDetail, logoUrl: string | undefined): string {
  return logoUrl
    ? `<img class="item-title-logo" src="${escapeHtml(logoUrl)}" alt="${escapeHtml(item.display_title)}" />`
    : `<h2 class="item-title-fallback">${escapeHtml(item.display_title)}</h2>`;
}

function renderSelectedItemHeroMeta(item: MediaItemDetail, genres: string[]): string {
  const tags = [
    missingItemDetailBadgeMarkup(item),
    playbackDetailBadgeMarkup(item),
  ];
  if (item.release_year) {
    tags.push(`<span class="tag">${item.release_year}</span>`);
  }
  if (item.content_rating) {
    tags.push(`<span class="tag">${escapeHtml(item.content_rating)}</span>`);
  }
  if (typeof item.rating === 'number') {
    tags.push(`<span class="tag">${escapeHtml(item.rating.toFixed(1))}</span>`);
  }
  tags.push(...genres.map((genre) => `<span class="tag">${escapeHtml(genre)}</span>`));

  return `<div class="hero-meta-row">${tags.join('')}</div>`;
}

function renderResumeButton(item: MediaItemDetail, resumeMs: number): string {
  if (!item.playable || resumeMs <= 0) {
    return '';
  }

  return `<button type="button" data-play-selected-item-start-ms="${resumeMs}">${renderButtonContent(`Resume ${formatDuration(resumeMs)}`, 'play')}</button>`;
}

function renderPrimaryPlayButton(item: MediaItemDetail, resumeMs: number): string {
  if (!item.playable) {
    return '';
  }

  const playButtonClass = resumeMs > 0 ? 'secondary-button' : '';
  const playButtonLabel = resumeMs > 0 ? 'Start over' : 'Play now';
  return `<button type="button" class="${playButtonClass}" data-play-selected-item-start-ms="0">${renderButtonContent(playButtonLabel, 'play')}</button>`;
}

function renderTrailerActionButton(preferredTrailer: TrailerOption | undefined, trailerButtonTitle: string): string {
  return preferredTrailer
    ? `<button type="button" class="secondary-button" id="play-item-trailer" title="${escapeHtml(trailerButtonTitle)}">${renderButtonContent('Play Trailer', 'play')}</button>`
    : '';
}

function renderThemeSongButton(themeSongOption: ReturnType<typeof currentThemeSongYouTubeTarget>): string {
  return themeSongOption
    ? `<button type="button" class="secondary-button" id="play-youtube-theme-song">${renderButtonContent('Play Theme', 'volume-2')}</button>`
    : '';
}

function renderSelectedItemActions(
  item: MediaItemDetail,
  resumeMs: number,
  playbackTarget: MediaPlaybackTarget | undefined,
  restartPlaybackTarget: MediaPlaybackTarget | undefined,
  preferredTrailer: TrailerOption | undefined,
  themeSongOption: ReturnType<typeof currentThemeSongYouTubeTarget>,
  trailerButtonTitle: string,
  backTarget: ReturnType<typeof backNavigationTarget>,
): string {
  return `
          <div class="detail-actions">
            ${renderResumeButton(item, resumeMs)}
            ${renderPrimaryPlayButton(item, resumeMs)}
            ${playbackTarget ? renderPlaybackTargetButton(playbackTarget, false) : ''}
            ${restartPlaybackTarget ? renderPlaybackTargetButton(restartPlaybackTarget, true) : ''}
            ${renderTrailerActionButton(preferredTrailer, trailerButtonTitle)}
            ${renderThemeSongButton(themeSongOption)}
            <button type="button" class="secondary-button" id="back-to-library">${renderButtonContent(backTarget.label, 'arrow-left')}</button>
          </div>
  `;
}

function renderTrailerPicker(trailerOptions: TrailerOption[], hasMultipleTrailers: boolean): string {
  if (!hasMultipleTrailers || !state.isTrailerMenuOpen) {
    return '';
  }

  return `
            <section class="trailer-picker panel">
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

function renderSelectedItemHero(
  item: MediaItemDetail,
  posterUrl: string | undefined,
  logoUrl: string | undefined,
  overview: string,
  genres: string[],
  actionsMarkup: string,
  trailerPickerMarkup: string,
): string {
  const itemHeroClass = item.item_type === 'episode' ? 'episode-hero' : '';
  const itemPosterClass = item.item_type === 'episode' ? 'item-thumbnail' : '';
  return `
      <section class="item-hero ${itemHeroClass}">
        <div class="detail-art item-poster ${itemPosterClass} ${posterUrl ? 'has-image' : ''}">
          ${renderSelectedItemPoster(item, posterUrl)}
        </div>
        <div class="detail-summary item-summary">
          ${renderSelectedItemTitle(item, logoUrl)}
          ${item.tagline ? `<p class="hero-tagline">${escapeHtml(item.tagline)}</p>` : ''}
          ${renderSelectedItemHeroMeta(item, genres)}
          ${renderCollapsibleText(overview, `item-overview:${item.id}`)}
          ${actionsMarkup}
          ${trailerPickerMarkup}
          <p class="muted">${escapeHtml(state.selectedPlayback?.reason ?? 'Loading playback capabilities…')}</p>
          ${renderSelectedItemFactList(item)}
        </div>
      </section>
  `;
}

function renderSelectedItemChildrenSection(item: MediaItemDetail): string {
  if (!item.children.length) {
    return '';
  }

  const childCountLabel = countLabel(item.children.length, 'item');
  const childGridClass = item.item_type === 'season' ? 'season-episodes-grid' : '';
  return `
        <section class="panel page-panel item-section">
          <div class="section-heading section-heading-actions">
            <h3>${escapeHtml(selectedItemChildSectionTitle(item))}</h3>
            <span class="muted">${childCountLabel}</span>
          </div>
          <div class="item-grid hierarchy-item-grid ${childGridClass}">${item.children.map(renderItemCard).join('')}</div>
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
  return `<button type="button" class="secondary-button" id="refresh-item-metadata" ${refreshButtonDisabled}>${renderButtonContent(refreshButtonLabel, 'refresh-cw')}</button>`;
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
  return `
      <section class="item-support-grid">
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

        <section class="panel page-panel item-section item-link-panel">
          <div class="section-heading section-heading-actions">
            <h3>${supportsManualLinking ? 'Link metadata' : 'Metadata'}</h3>
            ${metadataRefreshButtonMarkup}
          </div>
          ${renderLinkedMetadataSummary()}
          ${metadataSearchPanel}
        </section>
      </section>
  `;
}

function renderSelectedItemPage(item: MediaItemDetail): string {
  const trailerOptions = currentTrailerOptions();
  const hasMultipleTrailers = trailerOptions.length > 1;
  const supportsManualLinking = canManuallyLinkMetadata(item);
  const linkedMatch = state.selectedItemMetadata?.matches[0];
  const metadataRefreshActive = itemHasActiveMetadataRefresh(item);
  const resumeMs = resumablePlaybackPositionMs(item);
  const trailerButtonTitle = hasMultipleTrailers
    ? 'Click to play the first trailer. Right-click or press and hold to choose another trailer.'
    : 'Play Trailer';
  const actionsMarkup = renderSelectedItemActions(
    item,
    resumeMs,
    !item.playable ? item.playback_target ?? undefined : undefined,
    !item.playable ? item.restart_playback_target ?? undefined : undefined,
    trailerOptions[0],
    currentThemeSongYouTubeTarget(),
    trailerButtonTitle,
    backNavigationTarget(),
  );
  const metadataRefreshButtonMarkup = renderMetadataRefreshButton(supportsManualLinking, linkedMatch, metadataRefreshActive);

  return `
    <section class="item-page">
      ${renderSelectedItemBreadcrumbs(item)}
      ${renderSelectedItemHero(
        item,
        selectedItemPosterUrl(item),
        selectedItemLogoUrl(item),
        selectedItemOverview(item),
        item.genres.length ? item.genres : [],
        actionsMarkup,
        renderTrailerPicker(trailerOptions, hasMultipleTrailers),
      )}

      ${renderPeopleRail()}

      ${renderItemExtrasRail()}

      ${renderSelectedItemChildrenSection(item)}

      ${renderSelectedItemCollectionRails()}

      ${renderSelectedItemSupportGrid(
        item,
        state.libraries.find((entry) => entry.id === item.library_id),
        supportsManualLinking,
        metadataRefreshButtonMarkup,
        renderMetadataSearchPanel(supportsManualLinking),
      )}
    </section>
  `;
}

export function renderItemPage(): string {
  if (!state.selectedItem) {
    return '<section class="panel page-panel"><div class="empty-state">Loading item details…</div></section>';
  }

  return renderSelectedItemPage(state.selectedItem);
}
