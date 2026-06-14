/** Fallback YouTube ID used when a theme-song player must be created before a target video is known. */
export const YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID = 'dQw4w9WgXcQ';

/** Number of cards rendered initially for lazy home shelves. */
export const HOME_SHELF_CHUNK_SIZE = 12;

/** Numeric states reported by the YouTube iframe player. */
export const YOUTUBE_PLAYER_STATE = {
  ended: 0,
  playing: 1,
  paused: 2,
  buffering: 3,
  cued: 5,
} as const;

/** Character count after which long descriptions get a disclosure control. */
export const COLLAPSIBLE_TEXT_LENGTH = 520;

/** Line count after which long descriptions get a disclosure control. */
export const COLLAPSIBLE_TEXT_LINE_COUNT = 6;
