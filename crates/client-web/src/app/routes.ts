/** Parses browser location state into typed app routes. */
import type { AppRoute, HomeBrowseTab, SettingsSection } from './types';

/** Returns the initial home tab for a route. */
export function defaultHomeTab(_route: AppRoute): HomeBrowseTab {
  return 'recommended';
}

/** Converts the current browser path into the web UI's route model. */
export function parseRoute(): AppRoute {
  const normalizedPath = window.location.pathname.replace(/\/+$/, '') || '/';

  const settingsMatch = normalizedPath.match(/^\/settings(?:\/(libraries|providers|scheduled|dashboard|logs))?$/);
  if (settingsMatch) {
    return { page: 'settings', section: (settingsMatch[1] as SettingsSection | undefined) ?? 'general' };
  }

  const itemMatch = normalizedPath.match(/^\/items\/(\d+)$/);
  if (itemMatch) {
    return { page: 'item', itemId: Number(itemMatch[1]) };
  }

  const personMatch = normalizedPath.match(/^\/people\/(\d+)$/);
  if (personMatch) {
    return { page: 'person', personId: Number(personMatch[1]) };
  }

  const libraryBrowseMatch = normalizedPath.match(/^\/libraries\/(\d+)\/items\/(collections|categories|playlists)\/(.+)$/);
  if (libraryBrowseMatch) {
    return {
      page: 'browse-detail',
      libraryId: Number(libraryBrowseMatch[1]),
      kind: libraryBrowseMatch[2] === 'collections'
        ? 'collection'
        : libraryBrowseMatch[2] === 'playlists'
          ? 'playlist'
          : 'category',
      key: decodeURIComponent(libraryBrowseMatch[3]),
    };
  }

  const browseMatch = normalizedPath.match(/^\/items\/(collections|categories|playlists)\/(.+)$/);
  if (browseMatch) {
    return {
      page: 'browse-detail',
      kind: browseMatch[1] === 'collections'
        ? 'collection'
        : browseMatch[1] === 'playlists'
          ? 'playlist'
          : 'category',
      key: decodeURIComponent(browseMatch[2]),
    };
  }

  const libraryMatch = normalizedPath.match(/^\/libraries\/(\d+)$/);
  if (libraryMatch) {
    return { page: 'home', libraryId: Number(libraryMatch[1]) };
  }

  return { page: 'home' };
}
