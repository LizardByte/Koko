import type { YouTubeIframeApi } from './types';

let youtubeIframeApiPromise: Promise<YouTubeIframeApi> | undefined;

const YOUTUBE_VIDEO_ID_PATTERN = /^[A-Za-z0-9_-]{11}$/;
const PROTOCOL_RELATIVE_YOUTUBE_PREFIXES = [
  '//youtube.com/',
  '//www.youtube.com/',
  '//youtu.be/',
  '//youtube-nocookie.com/',
  '//www.youtube-nocookie.com/',
] as const;
const BARE_YOUTUBE_PREFIXES = [
  'youtube.com/',
  'www.youtube.com/',
  'youtu.be/',
  'youtube-nocookie.com/',
  'www.youtube-nocookie.com/',
] as const;
const YOUTUBE_VIDEO_PATH_KINDS = new Set(['embed', 'shorts', 'live']);

function validYouTubeVideoId(videoId: string | undefined): string | undefined {
  return YOUTUBE_VIDEO_ID_PATTERN.test(videoId ?? '') ? videoId : undefined;
}

function normalizedYouTubeParseTarget(url: string): string {
  if (PROTOCOL_RELATIVE_YOUTUBE_PREFIXES.some((prefix) => url.startsWith(prefix))) {
    return `https:${url}`;
  }

  if (BARE_YOUTUBE_PREFIXES.some((prefix) => url.startsWith(prefix))) {
    return `https://${url}`;
  }

  return url;
}

function isYouTubeHost(host: string): boolean {
  return host === 'youtube.com'
    || host.endsWith('.youtube.com')
    || host === 'youtube-nocookie.com'
    || host.endsWith('.youtube-nocookie.com');
}

function extractVideoIdFromParsedUrl(parsed: URL): string | undefined {
  const host = parsed.hostname.toLowerCase().replace(/^www\./, '');
  if (host === 'youtu.be') {
    return validYouTubeVideoId(parsed.pathname.split('/').filter(Boolean)[0]);
  }

  if (!isYouTubeHost(host)) {
    return undefined;
  }

  if (parsed.pathname.startsWith('/watch')) {
    return validYouTubeVideoId(parsed.searchParams.get('v')?.trim());
  }

  const [kind, videoId] = parsed.pathname.split('/').filter(Boolean);
  return YOUTUBE_VIDEO_PATH_KINDS.has(kind ?? '') ? validYouTubeVideoId(videoId) : undefined;
}

/** Extracts the canonical 11-character video ID from common YouTube URL formats. */
export function extractYouTubeVideoId(url: string): string | undefined {
  const normalizedUrl = url.trim();
  if (!normalizedUrl) {
    return undefined;
  }

  if (validYouTubeVideoId(normalizedUrl)) {
    return normalizedUrl;
  }

  try {
    return extractVideoIdFromParsedUrl(new URL(normalizedYouTubeParseTarget(normalizedUrl)));
  } catch {
    return undefined;
  }
}

/** Builds a normal YouTube watch URL when the input contains a valid video ID. */
export function buildYouTubeWatchUrl(url: string): string | undefined {
  const videoId = extractYouTubeVideoId(url);
  return videoId ? `https://www.youtube.com/watch?v=${videoId}` : undefined;
}

/** Loads and memoizes the YouTube iframe API script. */
export function loadYouTubeIframeApi(): Promise<YouTubeIframeApi> {
  if (globalThis.YT?.Player) {
    return Promise.resolve(globalThis.YT);
  }

  if (youtubeIframeApiPromise) {
    return youtubeIframeApiPromise;
  }

  youtubeIframeApiPromise = new Promise((resolve) => {
    const existingReadyHandler = globalThis.onYouTubeIframeAPIReady;
    globalThis.onYouTubeIframeAPIReady = () => {
      existingReadyHandler?.();
      if (globalThis.YT?.Player) {
        resolve(globalThis.YT);
      }
    };

    if (!document.querySelector<HTMLScriptElement>('script[src="https://www.youtube.com/iframe_api"]')) {
      const script = document.createElement('script');
      script.src = 'https://www.youtube.com/iframe_api';
      const firstScript = document.getElementsByTagName('script')[0];
      firstScript.parentNode?.insertBefore(script, firstScript);
    }
  });

  return youtubeIframeApiPromise;
}
