<script lang="ts">
  // SectionSupport — replaces renderSelectedItemSupportGrid()
  // (../client-web/src/app/itemPersonView.ts:990-1033). A 2-column grid wrapper
  // composing two independent panels: SupportFileInfo (file + library info)
  // and SupportMetadata (linked metadata summary). The panels share no state
  // beyond the item/metadata props, so they're separate components.
  import SupportFileInfo from './SupportFileInfo.svelte';
  import SupportMetadata from './SupportMetadata.svelte';
  import { libraries } from '$lib/stores';
  import type { MediaItemDetail, ItemMetadataResponse } from '$lib/api';

  type Props = {
    item: MediaItemDetail;
    metadata: ItemMetadataResponse | undefined;
  };
  let { item, metadata }: Props = $props();

  const library = $derived(libraries.byId(item.library_id));
</script>

<section class="item-support-grid">
  <SupportFileInfo {item} {library} />
  <SupportMetadata {item} {metadata} />
</section>

<style>
  /* 2-column grid wrapper only. Panel styling is shared (app.css); the panels'
     own rules live on SupportFileInfo/SupportMetadata. Mirrors vanilla
     style.css:1594-1708. */
  .item-support-grid {
    display: grid;
    grid-template-columns: minmax(260px, 360px) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }
</style>
