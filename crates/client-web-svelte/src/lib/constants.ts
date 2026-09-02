// Constants — port of ../client-web/src/app/constants.ts.

/**
 * No-operation function — for fire-and-forget promise rejections and
 * optional callback defaults. Avoids `no-empty-function` warnings while
 * remaining an explicit, named "this is intentionally a no-op" marker.
 */
// eslint-disable-next-line no-empty-function
export function noop(): void {}

/** Number of cards rendered initially for lazy home shelves. */
export const HOME_SHELF_CHUNK_SIZE = 12;

/** Character count after which long descriptions get a disclosure control. */
export const COLLAPSIBLE_TEXT_LENGTH = 520;

/** Line count after which long descriptions get a disclosure control. */
export const COLLAPSIBLE_TEXT_LINE_COUNT = 6;

/** Fallback YouTube ID used when a theme-song player must be created before a
 *  target video is known. (Used by the playback spike, not this phase.) */
export const YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID = 'dQw4w9WgXcQ';

/** Settings section ids + labels + paths — drives the settings sub-nav. */
export const SETTINGS_SECTIONS = [
  { id: 'general', label: 'General', path: '/settings' },
  { id: 'libraries', label: 'Libraries', path: '/settings/libraries' },
  { id: 'providers', label: 'Providers', path: '/settings/providers' },
  { id: 'scheduled', label: 'Scheduled', path: '/settings/scheduled' },
  { id: 'dashboard', label: 'Dashboard', path: '/settings/dashboard' },
  { id: 'logs', label: 'Logs', path: '/settings/logs' },
] as const;

/** Home browse tabs — drives the home tab nav. */
export const HOME_TABS: ReadonlyArray<{ id: 'recommended' | 'library' | 'collections' | 'playlists' | 'categories'; label: string }> = [
  { id: 'recommended', label: 'Recommended' },
  { id: 'library', label: 'Library' },
  { id: 'collections', label: 'Collections' },
  { id: 'playlists', label: 'Playlists' },
  { id: 'categories', label: 'Categories' },
];
