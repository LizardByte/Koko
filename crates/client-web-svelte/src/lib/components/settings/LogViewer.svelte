<script lang="ts">
  // LogViewer — structured log viewer with filters + entries table. Port of
  // renderLogViewer (dashboardView.ts:260-332). Data layer is already in the
  // activities store (logsResponse, logFilters, loadLogs, setLogFilters,
  // clearLogFilters).
  import { onMount } from 'svelte';
  import Button from '../Button.svelte';
  import { activities, ui } from '$lib/stores';

  onMount(() => {
    if (!activities.logsResponse && !activities.loading) {
      activities.loadLogs().catch(() => {});
    }
  });

  // Local filter form state (applied on submit, not live).
  let fLevel = $state('');
  let fModule = $state('');
  let fSince = $state('');
  let fUntil = $state('');
  let fSearch = $state('');

  // Sync local state from store on mount.
  $effect(() => {
    fLevel = activities.logFilters.level;
    fModule = activities.logFilters.module;
    fSince = activities.logFilters.since;
    fUntil = activities.logFilters.until;
    fSearch = activities.logFilters.search;
  });

  const LEVELS = ['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'];

  async function applyFilters(event: SubmitEvent) {
    event.preventDefault();
    activities.setLogFilters({ level: fLevel, module: fModule, since: fSince, until: fUntil, search: fSearch });
    try {
      await activities.loadLogs();
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to load logs.');
    }
  }

  async function clearFilters() {
    activities.clearLogFilters();
    fLevel = ''; fModule = ''; fSince = ''; fUntil = ''; fSearch = '';
    try {
      await activities.loadLogs();
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to load logs.');
    }
  }

  async function refreshLogs() {
    try {
      await activities.loadLogs();
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to refresh logs.');
    }
  }

  const entries = $derived(activities.logsResponse?.entries ?? []);
</script>

<section class="panel page-panel settings-log-panel">
  <div class="section-heading section-heading-actions">
    <div>
      <h3>Logs</h3>
      <p class="muted">Structured logs from {activities.logsResponse?.log_path ?? 'the current log file'}.</p>
    </div>
    <Button variant="secondary" label="Refresh logs" icon="refresh-cw" onclick={refreshLogs} />
  </div>

  <form class="settings-form log-filter-form" onsubmit={applyFilters}>
    <div class="form-row log-filter-row">
      <label>Level
        <select bind:value={fLevel}>
          <option value="">All levels</option>
          {#each LEVELS as level}<option value={level}>{level}</option>{/each}
        </select>
      </label>
      <label>Module<input bind:value={fModule} placeholder="koko::web::routes::media" /></label>
    </div>
    <div class="form-row log-filter-row">
      <label>From<input type="datetime-local" bind:value={fSince} /></label>
      <label>Until<input type="datetime-local" bind:value={fUntil} /></label>
    </div>
    <label>Search<input bind:value={fSearch} placeholder="message text, source path, or module" /></label>
    <div class="page-actions">
      <Button type="submit" label="Apply filters" icon="search" />
      <Button variant="secondary" label="Clear filters" icon="x" onclick={clearFilters} />
    </div>
  </form>

  {#if entries.length === 0}
    <div class="empty-state tight">No log entries matched the current filters.</div>
  {:else}
    <div class="table-shell">
      <table class="data-table log-entries-table">
        <thead>
          <tr>
            <th>Time</th>
            <th>Level</th>
            <th>Module</th>
            <th>Source</th>
            <th>Message</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as entry, i (i)}
            <tr>
              <td>{entry.timestamp}</td>
              <td><span class="tag {entry.level === 'ERROR' ? 'danger-tag' : entry.level === 'WARN' ? 'warning' : ''}">{entry.level}</span></td>
              <td>{entry.module}</td>
              <td class="muted">{entry.source_file_path}{typeof entry.line_number === 'number' ? `:${entry.line_number}` : ''}</td>
              <td><pre class="log-entry-message">{entry.message}</pre></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
