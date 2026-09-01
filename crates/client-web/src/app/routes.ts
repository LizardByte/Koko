/** Parses browser location state into typed app routes. */
import type { AppRoute, HomeBrowseTab, SettingsSection } from './types';

/** Returns the initial home tab for a route. */
export function defaultHomeTab(_route: AppRoute): HomeBrowseTab {
  return 'recommended';
}

/** Maps a browse URL segment to its route-model kind. */
function browseKindFromSegment(segment: string): Extract<AppRoute, { page: 'browse-detail' }>['kind'] {
  if (segment === 'collections') {
    return 'collection';
  }
  if (segment === 'playlists') {
    return 'playlist';
  }
  return 'category';
}

/** Removes trailing slashes while preserving the root path. */
function normalizePath(pathname: string): string {
  let end = pathname.length;
  while (end > 0 && pathname[end - 1] === '/') {
    end -= 1;
  }
  return pathname.slice(0, end) || '/';
}

/** Converts the current browser path into the web UI's route model. */
export function parseRoute(): AppRoute {
  const normalizedPath = normalizePath(globalThis.location.pathname);

  const settingsMatch = /^\/settings(?:\/(libraries|providers|scheduled|dashboard|logs))?$/.exec(normalizedPath);
  if (settingsMatch) {
    return { page: 'settings', section: (settingsMatch[1] as SettingsSection | undefined) ?? 'general' };
  }

  const itemMatch = /^\/items\/(\d+)$/.exec(normalizedPath);
  if (itemMatch) {
    return { page: 'item', itemId: Number(itemMatch[1]) };
  }

  const personMatch = /^\/people\/(\d+)$/.exec(normalizedPath);
  if (personMatch) {
    return { page: 'person', personId: Number(personMatch[1]) };
  }

  const libraryBrowseMatch = /^\/libraries\/(\d+)\/items\/(collections|categories|playlists)\/(.+)$/.exec(normalizedPath);
  if (libraryBrowseMatch) {
    return {
      page: 'browse-detail',
      libraryId: Number(libraryBrowseMatch[1]),
      kind: browseKindFromSegment(libraryBrowseMatch[2]),
      key: decodeURIComponent(libraryBrowseMatch[3]),
    };
  }

  const browseMatch = /^\/items\/(collections|categories|playlists)\/(.+)$/.exec(normalizedPath);
  if (browseMatch) {
    return {
      page: 'browse-detail',
      kind: browseKindFromSegment(browseMatch[1]),
      key: decodeURIComponent(browseMatch[2]),
    };
  }

  const libraryMatch = /^\/libraries\/(\d+)$/.exec(normalizedPath);
  if (libraryMatch) {
    return { page: 'home', libraryId: Number(libraryMatch[1]) };
  }

  return { page: 'home' };
}
