<script lang="ts">
  // YouTubeIframe — reusable YouTube IFrame Player API wrapper. Loads the API
  // via youtube.ts, creates a player in a mount <div>, and exposes state via
  // callbacks. Cleanup via $effect return (Opportunity F).
  //
  // Used by TrailerPlayer (visible overlay) and ThemeSongPlayer (hidden audio).
  import { loadYouTubeIframeApi } from '$lib/youtube';
  import type { YouTubePlayer } from '$lib/playerTypes';
  import { onMount } from 'svelte';

  type Props = {
    videoId: string;
    autoplay?: boolean;
    loop?: boolean;
    controls?: boolean;
    onready?: (player: YouTubePlayer) => void;
    onstatechange?: (player: YouTubePlayer) => void;
    onerror?: (errorCode: number) => void;
  };

  let {
    videoId,
    autoplay = true,
    loop = false,
    controls = false,
    onready,
    onstatechange,
    onerror,
  }: Props = $props();

  let container = $state<HTMLDivElement | undefined>(undefined);
  let player: YouTubePlayer | undefined = $state(undefined);

  onMount(() => {
    if (!container) return;

    let destroyed = false;

    const playerVars: Record<string, number | string> = {
      autoplay: autoplay ? 1 : 0,
      controls: controls ? 1 : 0,
      disablekb: 1,
      fs: 0,
      iv_load_policy: 3,
      loop: loop ? 1 : 0,
      modestbranding: 1,
      playsinline: 1,
      rel: 0,
    };
    if (globalThis.location?.origin?.startsWith('http')) {
      playerVars.origin = globalThis.location.origin;
    }

    loadYouTubeIframeApi().then((api) => {
      if (destroyed || !container) return;

      // YT.Player takes an element ID (string), not a DOM element.
      // Generate a unique ID for this instance.
      const mountId = `yt-player-${Math.random().toString(36).slice(2, 10)}`;
      const mount = document.createElement('div');
      mount.id = mountId;
      container.appendChild(mount);

      player = new api.Player(mountId, {
        height: '100%',
        width: '100%',
        videoId,
        playerVars,
        events: {
          onReady: (event) => {
            player = event.target;
            if (autoplay) event.target.playVideo();
            onready?.(event.target);
          },
          onStateChange: () => {
            if (player) onstatechange?.(player);
          },
          onError: (event) => {
            onerror?.(event.data);
          },
        },
      });
    });

    return () => {
      destroyed = true;
      if (player) {
        try {
          player.pauseVideo();
          player.destroy();
        } catch {
          // iframe may already be removed.
        }
      }
    };
  });

  // Load a new video when videoId changes.
  $effect(() => {
    if (player && videoId) {
      try {
        player.loadVideoById(videoId);
      } catch {
        // player not ready yet.
      }
    }
  });
</script>

<div bind:this={container} class="youtube-iframe-container"></div>
