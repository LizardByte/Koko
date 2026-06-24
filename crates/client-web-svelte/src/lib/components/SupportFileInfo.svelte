<script lang="ts">
  // SupportFileInfo — Panel A of SectionSupport: "File and library" info.
  // Shows library name, folder count, source path, and last-modified time.
  // Self-contained; needs only the item + optional library lookup.
  // Replaces the left panel of renderSelectedItemSupportGrid()
  // (../client-web/src/app/itemPersonView.ts:990-1033).
  import { formatTimestamp } from '$lib';
  import type { MediaItemDetail, MediaLibrary } from '$lib/api';

  type Props = { item: MediaItemDetail; library?: MediaLibrary };
  let { item, library }: Props = $props();
</script>

<div class="panel page-panel detail-card">
  <div class="section-heading"><h3>File and library</h3></div>
  <div class="item-info-list">
    <div><span class="label">Library</span><span>{library?.name ?? 'Unknown'}</span></div>
    <div><span class="label">Folders</span><span>{library?.paths.length ?? 0}</span></div>
    <div><span class="label">Source</span><span class="mono">{item.relative_path}</span></div>
    <div><span class="label">Updated</span><span>{formatTimestamp(item.modified_at)}</span></div>
  </div>
</div>

<style>
  .item-info-list {
    display: grid;
    gap: 0.9rem;
  }

  .item-info-list > div {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .mono {
    font-family: 'Cascadia Mono', 'Fira Code', Consolas, monospace;
    font-size: 0.82rem;
    word-break: break-all;
  }
</style>
