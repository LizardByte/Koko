import type { YouTubeIframeApi } from './types';

let youtubeIframeApiPromise: Promise<YouTubeIframeApi> | undefined;

/** Extracts the canonical 11-character video ID from common YouTube URL formats. */
export function extractYouTubeVideoId(url: string): string | undefined {
  const normalizedUrl = url.trim();
  if (!normalizedUrl) {
    return undefined;
  }

  const videoIdPattern = /^[A-Za-z0-9_-]{11}$/;
  if (videoIdPattern.test(normalizedUrl)) {
    return normalizedUrl;
  }

  let parseTarget = normalizedUrl;
  if (
    normalizedUrl.startsWith('//youtube.com/')
    || normalizedUrl.startsWith('//www.youtube.com/')
    || normalizedUrl.startsWith('//youtu.be/')
    || normalizedUrl.startsWith('//youtube-nocookie.com/')
    || normalizedUrl.startsWith('//www.youtube-nocookie.com/')
  ) {
    parseTarget = `https:${normalizedUrl}`;
  } else if (
    normalizedUrl.startsWith('youtube.com/')
    || normalizedUrl.startsWith('www.youtube.com/')
    || normalizedUrl.startsWith('youtu.be/')
    || normalizedUrl.startsWith('youtube-nocookie.com/')
    || normalizedUrl.startsWith('www.youtube-nocookie.com/')
  ) {
    parseTarget = `https://${normalizedUrl}`;
  }

  try {
    const parsed = new URL(parseTarget);
    const host = parsed.hostname.toLowerCase().replace(/^www\./, '');
    if (host === 'youtu.be') {
      const videoId = parsed.pathname.split('/').filter(Boolean)[0];
      return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
    }

    const isYouTubeHost = host === 'youtube.com'
      || host.endsWith('.youtube.com')
      || host === 'youtube-nocookie.com'
      || host.endsWith('.youtube-nocookie.com');
    if (isYouTubeHost) {
      if (parsed.pathname.startsWith('/watch')) {
        const videoId = parsed.searchParams.get('v')?.trim();
        return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
      }

      const [kind, videoId] = parsed.pathname.split('/').filter(Boolean);
      if (['embed', 'shorts', 'live'].includes(kind ?? '')) {
        return videoIdPattern.test(videoId ?? '') ? videoId : undefined;
      }
    }
  } catch {
    return undefined;
  }

  return undefined;
}

/** Builds a normal YouTube watch URL when the input contains a valid video ID. */
export function buildYouTubeWatchUrl(url: string): string | undefined {
  const videoId = extractYouTubeVideoId(url);
  return videoId ? `https://www.youtube.com/watch?v=${videoId}` : undefined;
}

/** Loads and memoizes the YouTube iframe API script. */
export function loadYouTubeIframeApi(): Promise<YouTubeIframeApi> {
  if (window.YT?.Player) {
    return Promise.resolve(window.YT);
  }

  if (youtubeIframeApiPromise) {
    return youtubeIframeApiPromise;
  }

  youtubeIframeApiPromise = new Promise((resolve) => {
    const existingReadyHandler = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      existingReadyHandler?.();
      if (window.YT?.Player) {
        resolve(window.YT);
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
