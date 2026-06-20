// Selectors for the item/person views — pure functions over store state,
// replacing the selector functions in ../client-web/src/app/selectors.ts and
// itemPersonView.ts. These take explicit arguments (not the global state
// singleton) so they're testable and Svelte-friendly.
import { getArtworkUrl, resolveApiUrl, type MediaCollectionSummary, type MediaItemDetail, type MediaItemSummary, type ItemMetadataResponse, type MediaLibrarySettings, type MediaLibrary } from './api';
import { libraries } from './stores/libraries.svelte';
import type { ItemMetadataPerson } from './api';

/** The item page's backdrop URL, or undefined when the item has none. */
export function pageBackdropUrlForItem(
  item: Pick<MediaItemSummary, 'id' | 'backdrop_url' | 'artwork_updated_at'> | undefined,
): string | undefined {
  return item?.backdrop_url ? getArtworkUrl(item.id, 'backdrop', item.artwork_updated_at) : undefined;
}

/** A collection's backdrop URL (artwork_url fallback) resolved via the API base. */
export function pageBackdropUrlForCollection(
  collection: Pick<MediaCollectionSummary, 'backdrop_url' | 'artwork_url'> | undefined,
): string | undefined {
  const url = collection?.backdrop_url ?? collection?.artwork_url;
  return url ? resolveApiUrl(url) : undefined;
}

/** "Back to <parent>" label — Back to library / season / show. */
export function backNavigationTarget(item: MediaItemDetail): { label: string; parentId?: number } {
  const parent = item.hierarchy[item.hierarchy.length - 1];
  if (!parent) {
    return { label: 'Back to library', parentId: item.library_id ? undefined : undefined };
  }
  const typeLabel =
    parent.item_type === 'show'
      ? 'show'
      : parent.item_type === 'season'
        ? 'season'
        : parent.item_type === 'movie'
          ? 'library'
          : 'library';
  if (typeLabel === 'library') {
    return { label: 'Back to library', parentId: parent.id };
  }
  return { label: `Back to ${typeLabel}`, parentId: parent.id };
}

/** Whether the item supports manual metadata linking (movies/shows only). */
export function canManuallyLinkMetadata(item: MediaItemDetail): boolean {
  return item.item_type === 'movie' || item.item_type === 'show';
}

/**
 * Match a settings-library entry to its persisted runtime library by path.
 * Mirrors persistedLibraryForSettings (../client-web/src/app/selectors.ts:42-50).
 * Used by the libraries settings page to show scan/refresh/delete-missing
 * actions + missing-items tags for persisted libraries.
 */
export function persistedLibraryForSettings(library: MediaLibrarySettings): MediaLibrary | undefined {
  const configuredPaths = new Set(
    [library.path, ...library.paths].map((path) => path.trim()).filter(Boolean),
  );
  return libraries.libraries.find((candidate) => {
    return configuredPaths.has(candidate.path) || candidate.paths.some((path) => configuredPaths.has(path));
  });
}

// --- Metadata search default-value helpers ---
// Mirror vanilla itemPersonView.ts:73-124. Used to pre-fill the metadata
// search form (query/year/language/providers) with sensible defaults.

import type { MetadataProviderStatus } from './api';

/**
 * Metadata providers eligible for manual linking on this item: configured,
 * implemented, non-secondary, and supporting the item's library kind.
 * Mirrors vanilla selectedItemMetadataProviderOptions (itemPersonView.ts:73-80).
 */
export function metadataProviderOptions(
  metadata: ItemMetadataResponse | undefined,
  libraryKind: string | undefined,
): MetadataProviderStatus[] {
  const providers = metadata?.providers ?? [];
  return providers
    .filter((p) => p.role !== 'secondary')
    .filter((p) => p.configured && p.implemented)
    .filter((p) => !libraryKind || p.supported_kinds.includes(libraryKind));
}

/** Default search query: the item's display_title, or the first match's title. */
export function defaultMetadataSearchTitle(
  item: MediaItemDetail,
  metadata: ItemMetadataResponse | undefined,
): string {
  return item.display_title.trim() || metadata?.matches[0]?.title?.trim() || '';
}

/** Default search year: the item's release_year, or the first match's. */
export function defaultMetadataSearchYear(
  item: MediaItemDetail,
  metadata: ItemMetadataResponse | undefined,
): string {
  const year = item.release_year ?? metadata?.matches[0]?.release_year;
  return typeof year === 'number' ? String(year) : '';
}

/** Default search language: the first provider's locale, or 'en'. */
export function defaultMetadataSearchLanguage(
  metadata: ItemMetadataResponse | undefined,
): string {
  return metadata?.matches[0]?.locale_key?.split('-')[0] || 'en';
}

/** The providers to pre-check in the search form (all eligible by default). */
export function defaultMetadataSearchProviderIds(
  metadata: ItemMetadataResponse | undefined,
  libraryKind: string | undefined,
): string[] {
  return metadataProviderOptions(metadata, libraryKind).map((p) => p.id);
}

/** The cast/people for the item, from the first metadata match's people array. */
export function selectedItemPeople(metadata: ItemMetadataResponse | undefined): ItemMetadataPerson[] {
  const people = metadata?.matches[0]?.people ?? [];
  return [...people].sort((a, b) => a.sort_order - b.sort_order);
}

/** Technical fact rows for the hero fact list. */
export function selectedItemTechnicalFacts(item: MediaItemDetail): Array<{ label: string; value: string }> {
  return [
    { label: 'Duration', value: formatDurationOrUnknown(item.duration_ms) },
    {
      label: 'Format',
      value:
        [item.container?.toUpperCase(), item.media_kind?.toUpperCase()].filter(Boolean).join(' • ') ||
        'Unknown',
    },
    {
      label: 'Codecs',
      value: [item.video_codec, item.audio_codec].filter(Boolean).join(' / ') || 'Unknown',
    },
    {
      label: 'Resolution',
      value: item.width && item.height ? `${item.width}×${item.height}` : 'Unknown',
    },
    { label: 'Bitrate', value: formatBitRateOrUnknown(item.bit_rate) },
    { label: 'Size', value: formatFileSizeOrUnknown(item.file_size) },
  ];
}

/** Collections that contain this item's root (minus the root itself). */
export function selectedItemCollectionRails(
  item: MediaItemDetail,
  collections: MediaCollectionSummary[],
): Array<{ collection: MediaCollectionSummary; items: MediaItemSummary[] }> {
  const rootId = item.hierarchy[0]?.id ?? item.id;
  const rails: Array<{ collection: MediaCollectionSummary; items: MediaItemSummary[] }> = [];
  for (const collection of collections) {
    if (collection.item_ids.includes(rootId) && collection.item_ids.length > 1) {
      // Note: in a full port we'd resolve item_ids to summaries via the catalog
      // store; for the PoC we surface the collection with its known ids.
      rails.push({ collection, items: [] });
    }
  }
  return rails;
}

// Local formatters to avoid pulling formatDuration etc. into a cycle — these
// are the same semantics as ../format.ts but inlined for the fact list.
function formatDurationOrUnknown(ms?: number): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms <= 0) return 'Unknown';
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}
function formatBitRateOrUnknown(bps?: number): string {
  if (typeof bps !== 'number' || !Number.isFinite(bps) || bps <= 0) return 'Unknown';
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`;
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(0)} kbps`;
  return `${bps} bps`;
}
function formatFileSizeOrUnknown(bytes?: number): string {
  if (typeof bytes !== 'number' || !Number.isFinite(bytes) || bytes <= 0) return 'Unknown';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(value >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

// Re-export so consumers don't need to know about the libraries store import.
export { libraries };

// ---------------------------------------------------------------------------
// Browse-detail selectors — ports of ../client-web/src/app/selectors.ts
// (categorySummaries, collectionSummaries, topLevelLibraryItems,
// rootAncestorForItem). Pure functions over the catalog's libraryItems, which
// BrowseListing.svelte and the Categories home tab consume.
// ---------------------------------------------------------------------------

/** Top-level (parentless) library items — the browseable roots. */
export function topLevelLibraryItems(items: MediaItemSummary[]): MediaItemSummary[] {
  return items.filter((item) => item.parent_id == null);
}

/** Walks parent_id chain to the root ancestor of an item. */
export function rootAncestorForItem(
  item: MediaItemSummary,
  itemsById: Map<number, MediaItemSummary>,
): MediaItemSummary {
  let current = item;
  while (typeof current.parent_id === 'number') {
    const parent = itemsById.get(current.parent_id);
    if (!parent) break;
    current = parent;
  }
  return current;
}

export interface CategorySummary {
  genre: string;
  count: number;
  items: MediaItemSummary[];
}

/**
 * Groups libraryItems by genre, deduplicating on each genre's root items
 * (so a show's episodes don't inflate a genre's count). Sorted by count desc
 * then genre name. Port of selectors.ts:135-163.
 */
export function categorySummaries(libraryItems: MediaItemSummary[]): CategorySummary[] {
  const itemsById = new Map(libraryItems.map((item) => [item.id, item]));
  const topLevelById = new Map(topLevelLibraryItems(libraryItems).map((item) => [item.id, item]));
  const categories = new Map<string, Map<number, MediaItemSummary>>();

  for (const item of libraryItems) {
    if (!item.genres.length) continue;
    const rootItem = rootAncestorForItem(item, itemsById);
    const root = topLevelById.get(rootItem.id) ?? rootItem;
    for (const genre of item.genres) {
      const normalized = genre.trim();
      if (!normalized) continue;
      if (!categories.has(normalized)) categories.set(normalized, new Map());
      categories.get(normalized)!.set(root.id, root);
    }
  }

  return [...categories.entries()]
    .map(([genre, items]) => ({ genre, count: items.size, items: [...items.values()] }))
    .sort((left, right) => right.count - left.count || left.genre.localeCompare(right.genre));
}

/** Items belonging to a collection (top-level items whose id is in item_ids). */
export function itemsForCollection(
  collection: MediaCollectionSummary,
  libraryItems: MediaItemSummary[],
): MediaItemSummary[] {
  const allowedIds = new Set(collection.item_ids);
  return topLevelLibraryItems(libraryItems).filter((item) => allowedIds.has(item.id));
}

