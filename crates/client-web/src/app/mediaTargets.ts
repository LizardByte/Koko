import { state } from './state';
import { collectionSummaries } from './selectors';
import type { TrailerOption } from './types';
import { extractYouTubeVideoId } from './youtube';
import { mediaExtraToTrailerOption } from './mediaExtras';

export function currentTrailerOptions(): TrailerOption[] {
  const options: TrailerOption[] = [];
  const seenUrls = new Set<string>();

  if (state.selectedItem?.trailer_url) {
    const url = state.selectedItem.trailer_url;
    seenUrls.add(url);
    options.push({
      title: state.selectedItem.trailer_title?.trim() || 'Trailer',
      url,
    });
  }

  for (const extra of state.selectedItem?.extras ?? []) {
    if (extra.extra_type !== 'trailer' || !extra.url || seenUrls.has(extra.url)) {
      continue;
    }

    seenUrls.add(extra.url);
    options.push(mediaExtraToTrailerOption(extra));
  }

  return options;
}

export function currentThemeSongTarget(): { title: string; url: string } | undefined {
  const route = state.route;
  if (route.page === 'browse-detail' && route.kind === 'collection') {
    const collection = collectionSummaries().find((entry) => entry.id === route.key);
    return collection?.theme_song_url
      ? { title: collection.name, url: collection.theme_song_url }
      : undefined;
  }

  if (route.page !== 'item' || !state.selectedItem?.theme_song_url) {
    return undefined;
  }

  return {
    title: state.selectedItem.display_title,
    url: state.selectedItem.theme_song_url,
  };
}

export function currentThemeSongYouTubeTarget(): { title: string; url: string; videoId: string } | undefined {
  const target = currentThemeSongTarget();
  const videoId = target ? extractYouTubeVideoId(target.url) : undefined;
  if (!target || !videoId) {
    return undefined;
  }

  return {
    title: target.title,
    url: target.url,
    videoId,
  };
}
