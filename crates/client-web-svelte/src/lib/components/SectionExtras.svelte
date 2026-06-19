<script lang="ts">
  // SectionExtras — replaces renderItemExtrasRail()
  // (../client-web/src/app/itemPersonView.ts:314-359). Thumbnails for trailers
  // and theme songs. Each card is a MediaExtraCard (shares the CardSurface
  // shell with MediaCard/PersonCard). Clicking dispatches into the (future)
  // playback controller.
  import MediaExtraCard from './MediaExtraCard.svelte';
  import { type MediaItemExtra } from '$lib/api';
  import { ui } from '$lib/stores';

  type Props = { extras: MediaItemExtra[] };
  let { extras }: Props = $props();

  const visible = $derived(extras.filter((extra) => extra.url));

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
