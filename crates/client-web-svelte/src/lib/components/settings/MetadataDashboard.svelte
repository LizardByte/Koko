<script lang="ts">
  // MetadataDashboard — summary tags + filter form + items table with clickable
  // column-header sorting. Port of renderMetadataDashboard (dashboardView.ts:
  // 110-215) + filter/summary logic (8-108). The sort is an enhancement beyond
  // vanilla (which sorts by refresh-state rank only); here the user can click
  // any column header to sort by it.
  import { onMount } from 'svelte';
  import Button from '../Button.svelte';
  import { goto } from '$app/navigation';
  import { activities, libraries } from '$lib/stores';
  import { formatTimestamp } from '$lib/format';
  import type { MediaItemSummary } from '$lib/api';

  onMount(() => {
    if (activities.dashboardItems.length === 0 && !activities.dashboardLoading) {
      activities.loadDashboard().catch(() => {});
    }
  });

  // --- Refresh-state derivation (dashboardView.ts:8-37) ---

  type RefreshState = 'pending' | 'stalled' | 'error' | 'fresh' | 'unmatched';

  function refreshState(item: MediaItemSummary): RefreshState {
    if (item.metadata_refresh_state === 'pending') {
      // Check if there's an active metadata-refresh activity for this item
      const hasActive = (activities.systemActivities?.activities ?? []).some(
        (a) => a.category === 'metadata_refresh' && a.state !== 'completed' && a.state !== 'failed',
      );
      return hasActive ? 'pending' : 'stalled';
    }
    if (item.metadata_refresh_state === 'error') return 'error';
    if (item.metadata_refresh_state === 'fresh' || item.has_metadata) return 'fresh';
    return 'unmatched';
  }

  function refreshLabel(state: RefreshState): string {
    switch (state) {
      case 'pending': return 'Refreshing';
      case 'stalled': return 'Pending without worker';
      case 'error': return 'Failed';
      case 'fresh': return 'Up to date';
      default: return 'Not linked';
    }
  }

  // --- Filtering (dashboardView.ts:39-76) ---

  const refreshRank: Record<RefreshState, number> = { error: 0, stalled: 1, pending: 2, unmatched: 3, fresh: 4 };

  const filteredItems = $derived.by(() => {
    const f = activities.metadataDashboardFilters;
    const search = f.search.trim().toLowerCase();
    return [...activities.dashboardItems]
      .filter((item) => {
        if (f.libraryId && String(item.library_id) !== f.libraryId) return false;
        if (f.itemType && item.item_type !== f.itemType) return false;
        if (f.refreshState && refreshState(item) !== f.refreshState) return false;
        if (search && !`${item.display_title} ${item.relative_path} ${item.metadata_refresh_error ?? ''}`.toLowerCase().includes(search)) return false;
        return true;
      });
  });

  // --- Sorting (enhancement: clickable headers) ---

  type SortColumn = 'title' | 'type' | 'library' | 'refreshState' | 'artwork';
  let sortColumn = $state<SortColumn>('refreshState');
  let sortDir = $state<'asc' | 'desc'>('asc');

  function toggleSort(column: SortColumn) {
    if (sortColumn === column) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortColumn = column;
      sortDir = 'asc';
    }
  }

  const sortedItems = $derived.by(() => {
    const items = [...filteredItems];
    const dir = sortDir === 'asc' ? 1 : -1;
    items.sort((a, b) => {
      switch (sortColumn) {
        case 'title': return dir * a.display_title.localeCompare(b.display_title);
        case 'type': return dir * a.item_type.localeCompare(b.item_type);
        case 'library': return dir * (a.library_id - b.library_id);
        case 'artwork': return dir * ((a.artwork_updated_at ?? 0) - (b.artwork_updated_at ?? 0));
        case 'refreshState':
        default:
          // Default: refresh-rank, then library, then title (vanilla order)
          return dir * (refreshRank[refreshState(a)] - refreshRank[refreshState(b)])
            || a.library_id - b.library_id
            || a.display_title.localeCompare(b.display_title);
      }
    });
    return items;
  });

  // --- Summary tags (dashboardView.ts:78-108) ---

  const summary = $derived.by(() => {
    const s = { failed: 0, pending: 0, stalled: 0, unmatched: 0 };
    for (const item of activities.dashboardItems) {
      const rs = refreshState(item);
      if (rs === 'error') s.failed++;
      else if (rs === 'pending') s.pending++;
      else if (rs === 'stalled') s.stalled++;
      else if (rs === 'unmatched') s.unmatched++;
    }
    return s;
  });

  // --- Filter form state ---

  let fLibrary = $state('');
  let fItemType = $state('');
  let fRefreshState = $state('');
  let fSearch = $state('');

  function applyFilters(event: SubmitEvent) {
    event.preventDefault();
    activities.setDashboardFilters({ libraryId: fLibrary, itemType: fItemType, refreshState: fRefreshState, search: fSearch });
  }

  function clearFilters() {
    fLibrary = ''; fItemType = ''; fRefreshState = ''; fSearch = '';
    activities.clearDashboardFilters();
  }

  function humanizeItemType(type: string): string {
    return type.charAt(0).toUpperCase() + type.slice(1).replace(/_/g, ' ');
  }

  const itemTypes = $derived([...new Set(activities.dashboardItems.map((i) => i.item_type))].sort());
</script>

<section class="panel page-panel metadata-dashboard-panel">
  <div class="section-heading section-heading-actions">
    <div>
      <h3>Metadata dashboard</h3>
      <p class="muted">Browse every item, identify failed refreshes, and spot pending items that no longer have an active worker.</p>
    </div>
    <div class="provider-tags">
      <span class="tag">{activities.dashboardItems.length} total</span>
      <span class="tag {summary.failed ? 'danger-tag' : ''}">{summary.failed} failed</span>
      <span class="tag warning">{summary.pending} active</span>
      <span class="tag {summary.stalled ? 'warning' : ''}">{summary.stalled} stalled</span>
      <span class="tag">{summary.unmatched} unmatched</span>
    </div>
  </div>

  <form class="settings-form metadata-dashboard-filter-form" onsubmit={applyFilters}>
    <div class="form-row metadata-dashboard-filter-grid">
      <label>Library
        <select bind:value={fLibrary}>
          <option value="">All libraries</option>
          {#each libraries.libraries as lib}<option value={lib.id}>{lib.name}</option>{/each}
        </select>
      </label>
      <label>Item type
        <select bind:value={fItemType}>
          <option value="">All item types</option>
          {#each itemTypes as type}<option value={type}>{humanizeItemType(type)}</option>{/each}
        </select>
      </label>
      <label>Refresh state
        <select bind:value={fRefreshState}>
          <option value="">All states</option>
          <option value="error">Failed</option>
          <option value="stalled">Pending without worker</option>
          <option value="pending">Refreshing</option>
          <option value="fresh">Up to date</option>
          <option value="unmatched">Not linked</option>
        </select>
      </label>
    </div>
    <label>Search<input bind:value={fSearch} placeholder="Title, path, or refresh error" /></label>
    <div class="page-actions">
      <Button type="submit" label="Apply filters" icon="search" />
      <Button variant="secondary" label="Clear filters" icon="x" onclick={clearFilters} />
    </div>
  </form>

  {#if activities.dashboardLoading}
    <div class="empty-state tight">Loading items…</div>
  {:else if sortedItems.length === 0}
    <div class="empty-state tight">No items matched the current dashboard filters.</div>
  {:else}
    <div class="table-shell">
      <table class="data-table">
        <thead>
          <tr>
            <th><button type="button" class="sort-header" onclick={() => toggleSort('title')}>Title {#if sortColumn === 'title'}{sortDir === 'asc' ? '↑' : '↓'}{/if}</button></th>
            <th><button type="button" class="sort-header" onclick={() => toggleSort('type')}>Type {#if sortColumn === 'type'}{sortDir === 'asc' ? '↑' : '↓'}{/if}</button></th>
            <th><button type="button" class="sort-header" onclick={() => toggleSort('library')}>Library {#if sortColumn === 'library'}{sortDir === 'asc' ? '↑' : '↓'}{/if}</button></th>
            <th><button type="button" class="sort-header" onclick={() => toggleSort('refreshState')}>Refresh state {#if sortColumn === 'refreshState'}{sortDir === 'asc' ? '↑' : '↓'}{/if}</button></th>
            <th><button type="button" class="sort-header" onclick={() => toggleSort('artwork')}>Artwork updated {#if sortColumn === 'artwork'}{sortDir === 'asc' ? '↑' : '↓'}{/if}</button></th>
            <th>Children</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each sortedItems as item (item.id)}
            {@const rs = refreshState(item)}
            {@const lib = libraries.byId(item.library_id)}
            <tr>
              <td>
                <div class="table-title-cell">
                  <strong>{item.display_title}</strong>
                  <p class="muted metadata-dashboard-path">{item.relative_path}</p>
                  {#if item.metadata_refresh_error}<p class="metadata-dashboard-error">{item.metadata_refresh_error}</p>{/if}
                </div>
              </td>
              <td>{humanizeItemType(item.item_type)}</td>
              <td>{lib?.name ?? `Library ${item.library_id}`}</td>
              <td>
                <span class="tag {rs === 'error' ? 'danger-tag' : rs === 'pending' || rs === 'stalled' ? 'warning' : rs === 'fresh' ? 'success' : ''}">
                  {refreshLabel(rs)}
                </span>
              </td>
              <td>{formatTimestamp(item.artwork_updated_at)}</td>
              <td>{item.child_count ?? ''}</td>
              <td><Button variant="secondary" label="Open item" icon="arrow-left" iconPosition="end" onclick={() => goto(`/items/${item.id}`)} /></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .sort-header {
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    padding: 0;
    text-align: left;
    white-space: nowrap;
  }
  .sort-header:hover {
    text-decoration: underline;
  }
</style>
