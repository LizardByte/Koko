<script lang="ts">
  // Libraries list — a real settings view using the mock data. Replaces the
  // library list portion of renderSettingsPage() (../client-web/src/app/
  // settingsView.ts). Add/delete library actions are out of scope for the PoC
  // (they're form -> POST handlers in the vanilla client); this demonstrates
  // the list rendering + status surface.
  import { onMount } from 'svelte';
  import { getLibraries, type MediaLibrary } from '$lib/api';
  import Tag from '$lib/components/Tag.svelte';
  import Spinner from '$lib/components/Spinner.svelte';

  let libraries = $state<MediaLibrary[]>([]);
  let loading = $state(true);
  let error = $state<string | undefined>(undefined);

  async function load() {
    loading = true;
    try {
      libraries = await getLibraries();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function statusTone(status: string): 'default' | 'warning' | 'info' {
    if (status === 'scanning') return 'info';
    if (status === 'error') return 'warning';
    return 'default';
  }
</script>

<section class="panel page-panel">
  <div class="section-heading">
    <div><h3>Libraries</h3><p class="muted">Configured media libraries.</p></div>
    <button type="button" onclick={load}>Refresh</button>
  </div>

  {#if loading && libraries.length === 0}
    <Spinner />
  {:else if error}
    <div class="empty-state">Failed to load libraries: {error}</div>
  {:else}
    <div class="table-shell">
      <table class="data-table">
        <thead>
          <tr><th>Name</th><th>Kind</th><th>Status</th><th>Files</th><th>Path</th></tr>
        </thead>
        <tbody>
          {#each libraries as lib (lib.id)}
            <tr>
              <td>{lib.name}</td>
              <td><span class="kind">{lib.kind}</span></td>
              <td><Tag variant={statusTone(lib.status)}>{lib.status}</Tag></td>
              <td>{lib.total_files.toLocaleString()}</td>
              <td class="muted mono">{lib.path}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .panel {
    background: var(--koko-surface, #fff);
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }
  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.8rem;
  }
  .section-heading h3 {
    margin: 0;
  }
  button {
    padding: 0.4rem 0.8rem;
    border: 1px solid var(--koko-border, #ddd);
    border-radius: 6px;
    background: var(--koko-surface, #fff);
    color: inherit;
    cursor: pointer;
  }
  button:hover {
    background: rgba(127, 127, 127, 0.08);
  }
  .table-shell {
    overflow-x: auto;
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }
  .data-table th,
  .data-table td {
    text-align: left;
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--koko-border, #ddd);
  }
  .kind {
    font-family: monospace;
    font-size: 0.82rem;
    background: rgba(127, 127, 127, 0.12);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
  }
  .mono {
    font-family: monospace;
    font-size: 0.82rem;
  }
  .muted {
    color: var(--koko-muted, #777);
  }
  .empty-state {
    color: var(--koko-muted, #777);
    padding: 1.5rem;
    text-align: center;
  }
</style>
