<script lang="ts">
  // SectionExtras — replaces renderItemExtrasRail()
  // (../client-web/src/app/itemPersonView.ts:314-359). Thumbnails for trailers
  // and theme songs. Each card is a MediaExtraCard (shares the CardSurface
  // shell with MediaCard/PersonCard). Clicking dispatches into the (future)
  // playback controller.
  import MediaExtraCard from './MediaExtraCard.svelte';
  import { type MediaItemExtra } from '$lib/api';
  import { playback } from '$lib/stores';
  import { mediaExtraToTrailerOption } from '$lib/mediaExtras';
  import { extractYouTubeVideoId } from '$lib/youtube';

  type Props = { extras: MediaItemExtra[] };
  let { extras }: Props = $props();

  const visible = $derived(extras.filter((extra) => extra.url));

  function play(extra: MediaItemExtra) {
    // If the extra is a YouTube URL, open it in the trailer overlay.
    // Otherwise, it's a direct media file — for now, still surface as not
    // implemented (requires a session for the extra's item).
    if (extra.url && extractYouTubeVideoId(extra.url)) {
      playback.openTrailer(mediaExtraToTrailerOption(extra));
    } else {
      // Non-YouTube extras would need their own playback session.
      // For now, open as a trailer (direct URL playback via the overlay).
      playback.openTrailer(mediaExtraToTrailerOption(extra));
    }
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
        <MediaExtraCard {extra} onplay={play} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .item-section .section-heading {
    margin-bottom: 0.6rem;
  }
</style>
