// Player type definitions — ported from vanilla client-web/src/app/types.ts.
// These are the player-specific types not already in api.ts.

/** A selectable trailer option (item-level trailer or trailer-type extra). */
export interface TrailerOption {
  title: string;
  url: string;
  label?: string;
  titleSuffix?: string;
}

/** A resolved theme-song source — either a direct audio URL or YouTube. */
export type ThemeSongSource =
  | { kind: 'audio'; src: string; title: string }
  | { kind: 'youtube'; src: string; title: string; videoId: string };

/** Minimal YouTube iframe player contract used by the client. */
export interface YouTubePlayer {
  loadVideoById(videoId: string): void;
  playVideo(): void;
  pauseVideo(): void;
  seekTo(seconds: number, allowSeekAhead: boolean): void;
  getCurrentTime(): number;
  getDuration(): number;
  getPlayerState(): number;
  setVolume(volume: number): void;
  getVolume(): number;
  mute(): void;
  unMute(): void;
  isMuted(): boolean;
  setPlaybackQuality(suggestedQuality: string): void;
  destroy(): void;
}

/** Browser global exposed after loading the YouTube iframe API script. */
export interface YouTubeIframeApi {
  Player: new (
    elementId: string,
    options: {
      height: string;
      width: string;
      videoId?: string;
      playerVars?: Record<string, number | string>;
      events?: {
        onReady?: (event: { target: YouTubePlayer }) => void;
        onStateChange?: () => void;
        onError?: (event: { data: number }) => void;
      };
    },
  ) => YouTubePlayer;
}

// YouTube player state constants (YT.PlayerState values).
export const YOUTUBE_PLAYER_STATE = {
  unstarted: -1,
  ended: 0,
  playing: 1,
  paused: 2,
  buffering: 3,
  cued: 5,
} as const;

// Placeholder video used while the theme-song player initializes (vanilla
// uses this to avoid loading a real video before the target is known).
export const YOUTUBE_THEME_PLACEHOLDER_VIDEO_ID = 'jNQXAC9IVRw';

declare global {
  var YT: YouTubeIframeApi | undefined;
  var onYouTubeIframeAPIReady: (() => void) | undefined;
}
