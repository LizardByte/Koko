// Media targets — determines available trailer options + theme-song target.
// Ported from vanilla client-web/src/app/mediaTargets.ts (63 lines), adapted
// to read from the item store + page state instead of global `state`.

import type { MediaItemDetail } from './api';
import type { TrailerOption, ThemeSongSource } from './playerTypes';
import { extractYouTubeVideoId } from './youtube';
import { mediaExtraToTrailerOption } from './mediaExtras';

/** Collects all trailer options for an item: item-level trailer + trailer extras. */
export function currentTrailerOptions(item: MediaItemDetail | undefined): TrailerOption[] {
  if (!item) return [];
  const options: TrailerOption[] = [];
  const seenUrls = new Set<string>();

  if (item.trailer_url) {
    const url = item.trailer_url;
    seenUrls.add(url);
    options.push({
      title: item.trailer_title?.trim() || 'Trailer',
      url,
    });
  }

  for (const extra of item.extras ?? []) {
    if (extra.extra_type !== 'trailer' || !extra.url || seenUrls.has(extra.url)) {
      continue;
    }
    seenUrls.add(extra.url);
    options.push(mediaExtraToTrailerOption(extra));
  }

  return options;
}

/**
 * Determines the theme-song target for the current view. On item pages, uses
 * the item's theme_song_url. On collection browse pages, uses the collection's
 * theme_song_url. Returns undefined when no theme song is available or when
 * the player/trailer is active.
 */
export function currentThemeSongTarget(
  item: MediaItemDetail | undefined,
  collectionThemeSongUrl?: string,
  collectionName?: string,
): { title: string; url: string } | undefined {
  // Collection browse page
  if (collectionThemeSongUrl) {
    return { title: collectionName ?? 'Collection', url: collectionThemeSongUrl };
  }

  // Item page
  if (!item?.theme_song_url) {
    return undefined;
  }

  return {
    title: item.display_title,
    url: item.theme_song_url,
  };
}

/** Resolves a theme-song URL into a typed source (audio or YouTube). */
export function themeSongSourceFromUrl(
  themeSongUrl: string,
  title: string,
): ThemeSongSource | undefined {
  if (!themeSongUrl) {
    return undefined;
  }

  const videoId = extractYouTubeVideoId(themeSongUrl);
  if (videoId) {
    return {
      kind: 'youtube',
      src: videoId,
      title,
      videoId,
    };
  }

  // Import resolveApiUrl lazily to avoid circular deps in tests
  return {
    kind: 'audio',
    src: themeSongUrl,
    title,
  };
}
