<script lang="ts">
  // ItemExtras — replaces renderItemExtrasRail() + renderMediaExtraCard()
  // (../client-web/src/app/itemPersonView.ts:314-359). Thumbnails for trailers
  // and theme songs. Clicking dispatches into the (future) playback controller.
  import Icon from './Icon.svelte';
  import { resolveApiUrl, type MediaItemExtra } from '$lib/api';
  import { ui } from '$lib/stores';

  type Props = { extras: MediaItemExtra[] };
  let { extras }: Props = $props();

  const visible = $derived(extras.filter((extra) => extra.url));

  function thumbnail(extra: MediaItemExtra): string | undefined {
    return extra.thumbnail_url ? resolveApiUrl(extra.thumbnail_url) : undefined;
  }

  function play(extra: MediaItemExtra) {
    ui.setError(`Playing "${extra.title ?? extra.extra_type}" is not yet implemented (playbackController spike).`);
  }
</script>

{#if visible.length}
  <section class="panel page-panel item-section">
    <div class="section-heading section-heading-actions">
      <div><h3>Extras</h3></div>
      <span class="muted">{visible.length}</span>
    </div>
    <div class="extras-row">
      {#each visible as extra, i (i)}
        <button type="button" class="media-extra-card" onclick={() => play(extra)} title={extra.title}>
          <span class="media-extra-thumbnail" class:has-image={Boolean(thumbnail(extra))} style={thumbnail(extra) ? `background-image: url('${thumbnail(extra)}');` : ''}>
            {#if !thumbnail(extra)}
              <span class="media-extra-placeholder"><Icon name={extra.extra_type === 'theme_song' ? 'volume-2' : 'play'} size={24} /></span>
            {/if}
            <span class="media-extra-play-icon"><Icon name="play" size={20} /></span>
          </span>
          <span class="media-extra-title">{extra.title ?? extra.extra_type}</span>
          <span class="media-extra-meta">
            <span>{extra.extra_type === 'theme_song' ? 'Theme' : 'Trailer'}</span>
            {#if extra.duration_seconds}<span>{Math.floor(extra.duration_seconds / 60)}m</span>{/if}
          </span>
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  .item-section .section-heading {
    margin-bottom: 0.6rem;
  }
  .media-extra-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0;
    background: transparent;
    box-shadow: none;
    text-align: left;
  }
  .media-extra-thumbnail {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: linear-gradient(180deg, rgba(93, 123, 255, 0.4), rgba(27, 37, 62, 0.9));
    background-size: cover;
    background-position: center;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.85);
  }
  .media-extra-placeholder {
    display: grid;
    place-items: center;
  }
  .media-extra-play-icon {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #fff;
    opacity: 0;
    transition: opacity 0.15s ease;
    background: rgba(0, 0, 0, 0.4);
  }
  .media-extra-card:hover .media-extra-play-icon {
    opacity: 1;
  }
  .media-extra-title {
    font-size: 0.82rem;
    font-weight: 600;
    color: #f4f7fb;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .media-extra-meta {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: #9ab1d1;
  }
</style>
