/** Provides derived state selectors for navigation, previews, and browse views. */
import type { MediaCollectionSummary, MediaItemSummary, MediaLibrary, MediaLibrarySettings } from '../api';
import { getArtworkUrl, resolveApiUrl } from '../api';
import { state } from './state';
import { humanizeItemType } from './ui';

export type HomeFeaturePreview =
  | { kind: 'collection'; collection: MediaCollectionSummary }
  | { kind: 'item'; item: MediaItemSummary };

export function activeLibraryId(): number | undefined {
  if (state.route.page === 'home' || state.route.page === 'browse-detail') {
    return state.route.libraryId;
  }

  return state.selectedItem?.library_id;
}

export function activeLibrary(): MediaLibrary | undefined {
  return state.libraries.find((library) => library.id === activeLibraryId());
}

export function activeLibrarySettings(): MediaLibrarySettings | undefined {
  const library = activeLibrary();
  if (!library || !state.settingsResponse) {
    return undefined;
  }

  const settingsWithPaths = state.settingsResponse.settings.media.libraries.map((settings) => {
    const paths = [settings.path, ...settings.paths].map((path) => path.trim()).filter(Boolean);
    return { settings, paths };
  });
  const pathMatch = settingsWithPaths.find(({ paths }) => {
    return paths.includes(library.path)
      || library.paths.some((path) => paths.includes(path));
  });

  return pathMatch?.settings
    ?? settingsWithPaths.find(({ settings }) => settings.name === library.name)?.settings;
}

export function persistedLibraryForSettings(library: MediaLibrarySettings): MediaLibrary | undefined {
  const configuredPaths = [library.path, ...library.paths]
    .map((path) => path.trim())
    .filter(Boolean);
  return state.libraries.find((candidate) => {
    return configuredPaths.includes(candidate.path)
      || candidate.paths.some((path) => configuredPaths.includes(path));
  });
}

export function canManuallyLinkMetadata(item?: MediaItemSummary): boolean {
  return item?.item_type === 'movie' || item?.item_type === 'show';
}

export function backNavigationTarget(): { label: string; path: string } {
  const hierarchy = state.selectedItem?.hierarchy ?? [];
  const parent = hierarchy[hierarchy.length - 1];
  if (parent) {
    return {
      label: `Back to ${humanizeItemType(parent.item_type).toLowerCase()}`,
      path: `/items/${parent.id}`,
    };
  }

  const libraryId = state.selectedItem?.library_id;
  return {
    label: 'Back to library',
    path: typeof libraryId === 'number' ? `/libraries/${libraryId}` : '/',
  };
}

export function topLevelLibraryItems(): MediaItemSummary[] {
  return state.libraryItems.filter((item) => item.parent_id == null);
}

export function rootItemById(): Map<number, MediaItemSummary> {
  return new Map(topLevelLibraryItems().map((item) => [item.id, item]));
}

export function mediaItemsById(): Map<number, MediaItemSummary> {
  return new Map(state.libraryItems.map((item) => [item.id, item]));
}

export function homePreviewItemsById(): Map<number, MediaItemSummary> {
  const items = [
    ...state.libraryItems,
    ...(state.home?.shelves ?? []).flatMap((shelf) => shelf.items),
    ...searchResultItems(),
  ];

  return new Map(items.map((item) => [item.id, item]));
}

export function rootAncestorForItem(item: MediaItemSummary, itemsById: Map<number, MediaItemSummary>): MediaItemSummary {
  let current = item;

  while (typeof current.parent_id === 'number') {
    const parent = itemsById.get(current.parent_id);
    if (!parent) {
      break;
    }
    current = parent;
  }

  return current;
}

export function showPreviewItemForHighlight(item: MediaItemSummary): MediaItemSummary {
  if (item.item_type !== 'season' && item.item_type !== 'episode') {
    return item;
  }

  const hierarchyShow = item.hierarchy?.find((ancestor) => ancestor.item_type === 'show');
  if (hierarchyShow) {
    return hierarchyShow;
  }

  const itemsById = homePreviewItemsById();
  let current = item;
  while (typeof current.parent_id === 'number') {
    const parent = itemsById.get(current.parent_id);
    if (!parent) {
      break;
    }
    if (parent.item_type === 'show') {
      return parent;
    }
    current = parent;
  }

  return item;
}

export function categorySummaries(): Array<{ genre: string; count: number; items: MediaItemSummary[] }> {
  const itemsById = mediaItemsById();
  const rootsById = rootItemById();
  const categories = new Map<string, Map<number, MediaItemSummary>>();

  state.libraryItems.forEach((item) => {
    if (!item.genres.length) {
      return;
    }

    const rootItem = rootAncestorForItem(item, itemsById);
    const root = rootsById.get(rootItem.id) ?? rootItem;
    item.genres.forEach((genre) => {
      const normalizedGenre = genre.trim();
      if (!normalizedGenre) {
        return;
      }

      if (!categories.has(normalizedGenre)) {
        categories.set(normalizedGenre, new Map());
      }
      categories.get(normalizedGenre)!.set(root.id, root);
    });
  });

  return [...categories.entries()]
    .map(([genre, items]) => ({ genre, count: items.size, items: [...items.values()] }))
    .sort((left, right) => right.count - left.count || left.genre.localeCompare(right.genre));
}

export function collectionSummaries(): MediaCollectionSummary[] {
  return state.home?.collections ?? [];
}

export function collectionForRoute(): MediaCollectionSummary | undefined {
  const route = state.route;
  if (route.page !== 'browse-detail' || route.kind !== 'collection') {
    return undefined;
  }

  return collectionSummaries().find((entry) => entry.id === route.key);
}

export function itemsForCollection(collection: MediaCollectionSummary): MediaItemSummary[] {
  const allowedIds = new Set(collection.item_ids);
  return topLevelLibraryItems().filter((item) => allowedIds.has(item.id));
}

export function selectedItemRoot(): MediaItemSummary | undefined {
  if (!state.selectedItem) {
    return undefined;
  }

  return state.selectedItem.hierarchy[0] ?? state.selectedItem;
}

export function selectedItemCollectionRails(): Array<{ collection: MediaCollectionSummary; items: MediaItemSummary[] }> {
  const root = selectedItemRoot();
  if (!root) {
    return [];
  }

  return collectionSummaries()
    .filter((collection) => collection.item_ids.includes(root.id))
    .map((collection) => ({
      collection,
      items: itemsForCollection(collection).filter((item) => item.id !== root.id),
    }))
    .filter((rail) => rail.items.length > 0);
}

export function categoryForRoute(): { genre: string; count: number; items: MediaItemSummary[] } | undefined {
  const route = state.route;
  if (route.page !== 'browse-detail' || route.kind !== 'category') {
    return undefined;
  }

  return categorySummaries().find((entry) => entry.genre === route.key);
}

export function browseItemsForRoute(): MediaItemSummary[] {
  const route = state.route;
  if (route.page !== 'browse-detail') {
    return [];
  }

  if (route.kind === 'collection') {
    const collection = collectionForRoute();
    return collection ? itemsForCollection(collection) : [];
  }

  if (route.kind === 'category') {
    return categoryForRoute()?.items ?? [];
  }

  return [];
}

export function filteredTopLevelLibraryItems(): MediaItemSummary[] {
  const items = topLevelLibraryItems();
  if (!state.browseFilter) {
    return items;
  }

  const allowedIds = new Set(state.browseFilter.itemIds);
  return items.filter((item) => allowedIds.has(item.id));
}

export function searchResultItems(): MediaItemSummary[] {
  return state.searchResults.flatMap((result) => result.result_type === 'item' ? [result.item] : []);
}

export function searchResultCollections(): MediaCollectionSummary[] {
  return state.searchResults.flatMap((result) => result.result_type === 'collection' ? [result.collection] : []);
}

export function homeSearchPreview(): HomeFeaturePreview | undefined {
  if (typeof state.homePreviewItemId === 'number') {
    const item = searchResultItems().find((entry) => entry.id === state.homePreviewItemId);
    if (item) {
      return { kind: 'item', item: showPreviewItemForHighlight(item) };
    }
  }
  if (state.homePreviewCollectionId) {
    const collection = searchResultCollections().find((entry) => entry.id === state.homePreviewCollectionId);
    if (collection) {
      return { kind: 'collection', collection };
    }
  }

  for (const result of state.searchResults) {
    if (result.result_type === 'item') {
      return { kind: 'item', item: showPreviewItemForHighlight(result.item) };
    }
    if (result.result_type === 'collection') {
      return { kind: 'collection', collection: result.collection };
    }
  }
  return undefined;
}

export function homeFeaturePreview(): HomeFeaturePreview | undefined {
  if (state.route.page === 'browse-detail' && state.route.kind === 'collection') {
    const collection = collectionForRoute();
    return collection ? { kind: 'collection', collection } : undefined;
  }

  if (state.route.page === 'home' && state.searchQuery.trim() && state.searchResults.length) {
    return homeSearchPreview();
  }

  if (state.route.page === 'home' && state.homeTab === 'collections') {
    const collections = collectionSummaries();
    const collection = collections.find((entry) => entry.id === state.homePreviewCollectionId) ?? collections[0];
    return collection ? { kind: 'collection', collection } : undefined;
  }

  const item = homePreviewItem();
  return item ? { kind: 'item', item } : undefined;
}

export function homePreviewItem(): MediaItemSummary | undefined {
  const items = homePreviewCandidates();
  if (!items.length) {
    return undefined;
  }

  return showPreviewItemForHighlight(items.find((item) => item.id === state.homePreviewItemId) ?? items[0]);
}

export function homePreviewCandidates(): MediaItemSummary[] {
  if (state.route.page === 'browse-detail') {
    return browseItemsForRoute();
  }

  if (state.route.page === 'home' && state.searchQuery.trim() && state.searchResults.length) {
    return searchResultItems();
  }

  switch (state.homeTab) {
    case 'library':
      return filteredTopLevelLibraryItems();
    case 'collections': {
      return filteredTopLevelLibraryItems();
    }
    case 'categories': {
      const seen = new Set<number>();
      const categoryItems = categorySummaries().flatMap((category) => category.items).filter((item) => {
        if (seen.has(item.id)) {
          return false;
        }
        seen.add(item.id);
        return true;
      });
      return categoryItems.length ? categoryItems : filteredTopLevelLibraryItems();
    }
    default: {
      const shelfItems = (state.home?.shelves ?? []).flatMap((shelf) => shelf.items);
      return shelfItems.length ? shelfItems : filteredTopLevelLibraryItems();
    }
  }
}

export function pageBackdropUrlForItem(item: Pick<MediaItemSummary, 'id' | 'backdrop_url' | 'artwork_updated_at'> | undefined): string | undefined {
  return item?.backdrop_url
    ? getArtworkUrl(item.id, 'backdrop', item.artwork_updated_at)
    : undefined;
}

export function pageBackdropUrlForCollection(collection: Pick<MediaCollectionSummary, 'backdrop_url' | 'artwork_url'> | undefined): string | undefined {
  const artworkUrl = collection?.backdrop_url ?? collection?.artwork_url;
  return artworkUrl ? resolveApiUrl(artworkUrl) : undefined;
}

export function pageBackdropUrlForHomePreview(preview: HomeFeaturePreview | undefined): string | undefined {
  if (!preview) {
    return undefined;
  }

  return preview.kind === 'collection'
    ? pageBackdropUrlForCollection(preview.collection)
    : pageBackdropUrlForItem(preview.item);
}
