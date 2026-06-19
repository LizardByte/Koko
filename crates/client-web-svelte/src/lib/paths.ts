// URL builders for browse-detail navigation. Port of browseDetailPath /
// homeBrowsePath from ../client-web/src/app/homeView.ts:41-67. Used by the
// collection/category/playlist cards to navigate to the browse-detail route.

export type BrowseListingKind = 'collection' | 'category' | 'playlist';

/** The URL segment for a browse-detail kind (collections/categories/playlists). */
function browseSegment(kind: BrowseListingKind): string {
  if (kind === 'collection') return 'collections';
  if (kind === 'playlist') return 'playlists';
  return 'categories';
}

/**
 * Browse-detail URL for a given kind+key, scoped to a library when one is
 * active. Matches vanilla browseDetailPath (homeView.ts:41-52): library-scoped
 * paths live under /libraries/:id/items/<segment>/<key>, library-less under
 * /items/<segment>/<key>.
 */
export function browseDetailPath(
  kind: BrowseListingKind,
  key: string,
  libraryId?: number,
): string {
  const segment = browseSegment(kind);
  const encodedKey = encodeURIComponent(key);
  return typeof libraryId === 'number'
    ? `/libraries/${libraryId}/items/${segment}/${encodedKey}`
    : `/items/${segment}/${encodedKey}`;
}

/** Home browse URL — `/libraries/:id` when a library is active, else `/`. */
export function homeBrowsePath(libraryId?: number): string {
  return typeof libraryId === 'number' ? `/libraries/${libraryId}` : '/';
}
