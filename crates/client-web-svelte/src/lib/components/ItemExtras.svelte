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
              <span class="media-extra-placeholder-icon"><Icon name={extra.extra_type === 'theme_song' ? 'music' : 'play'} size={32} /></span>
            {/if}
            <span class="media-extra-play-icon"><Icon name="play" size={16} /></span>
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
  /*
   * Component-owned. Values mirror vanilla style.css:1745-1817.
   * .extras-row is shared (app.css); everything else here is ItemExtras-only.
   */
  .item-section .section-heading {
    margin-bottom: 0.6rem;
  }

  .media-extra-card {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    align-items: stretch;
    width: 244px;
    padding: 0;
    border-radius: 8px;
    background: transparent;
    box-shadow: none;
    text-align: left;
  }

  .media-extra-thumbnail {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 16 / 9;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.08);
    /* background-size/position/repeat apply when an inline background-image
       (thumbnail) is present, making it cover the box like <img object-fit>.
       The gradient fallback is layered underneath via :not(.has-image) so the
       image, when present, sits on top and fully covers. */
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
    color: #e7f0ff;
  }

  /* Gradient fallback — shown only when there's no thumbnail image. Defined
     as a layer so an inline background-image (when .has-image) overrides it. */
  .media-extra-thumbnail:not(.has-image) {
    background-image: linear-gradient(135deg, rgba(57, 78, 123, 0.88), rgba(12, 18, 32, 0.94));
  }

  .media-extra-placeholder-icon :global(svg) {
    width: 2rem;
    height: 2rem;
  }

  .media-extra-play-icon {
    position: absolute;
    right: 0.55rem;
    bottom: 0.55rem;
    display: inline-grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    background: rgba(5, 10, 18, 0.72);
    color: #fff;
  }

  .media-extra-title {
    min-height: 2.5rem;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    font-weight: 700;
    line-height: 1.25;
  }

  .media-extra-meta {
    display: flex;
    justify-content: space-between;
    gap: 0.65rem;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .media-extra-meta span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
