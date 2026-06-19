<script lang="ts">
  // The logs view, ported from ../client-web/src/app/dashboardView.ts renderLogViewer().
  //
  // What this demonstrates vs the vanilla client:
  //  - state.logFilters + state.logsResponse (two slices of the global singleton)
  //    become local page-level $state runes.
  //  - The filter form's imperative FormData-reading + render() calls
  //    (eventBindings.ts handlers) become Svelte bind:value + a single load
  //    function. No domPatcher, no eventBindings, no AbortController plumbing.
  //  - The level->tag-class mapping and the table structure are unchanged.
  import { onMount } from 'svelte';
  import {
    getLogs,
    EMPTY_LOG_FILTERS,
    type LogEntriesResponse,
    type LogFilters,
    type LogEntry,
  } from '$lib/api';
  import { buildLogFilterRequest } from '$lib/activities';

  // Local page state — replaces state.logFilters / state.logsResponse.
  let filters = $state<LogFilters>({ ...EMPTY_LOG_FILTERS });
  let response = $state<LogEntriesResponse | undefined>(undefined);
  let loading = $state(false);
  let error = $state<string | undefined>(undefined);

  // A working copy of the filters bound to the form, applied on submit.
  // Mirrors how the vanilla client only commits filters on form submit.
  let draft = $state<LogFilters>({ ...EMPTY_LOG_FILTERS });

  // $derived replaces filteredMetadataDashboardItems-style selectors.
  const entries = $derived<LogEntry[]>(response?.entries ?? []);
  const logPath = $derived(response?.log_path ?? 'the current log file');

  async function load() {
    loading = true;
    error = undefined;
    try {
      response = await getLogs(buildLogFilterRequest(filters));
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function applyFilters(event: SubmitEvent) {
    event.preventDefault();
    filters = { ...draft, level: draft.level.toUpperCase() };
    load();
  }

  function clearFilters() {
    draft = { ...EMPTY_LOG_FILTERS };
    filters = { ...EMPTY_LOG_FILTERS };
    load();
  }

  function levelTagClass(level: string): string {
    if (level === 'ERROR') return 'danger-tag';
    if (level === 'WARN') return 'warning';
    return '';
  }

  function sourceCell(entry: LogEntry): string {
    return entry.source_file_path + (typeof entry.line_number === 'number' ? `:${entry.line_number}` : '');
  }

  onMount(load);

  const LEVELS = ['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'];
</script>

<section class="panel page-panel settings-log-panel">
  <div class="section-heading section-heading-actions">
    <div>
      <h3>Logs</h3>
      <p class="muted">Structured logs from {logPath}.</p>
    </div>
    <button type="button" onclick={load}>Refresh logs</button>
  </div>

  <form class="settings-form log-filter-form" onsubmit={applyFilters}>
    <div class="form-row log-filter-row">
      <label>
        Level
        <select bind:value={draft.level}>
          <option value="">All levels</option>
          {#each LEVELS as level (level)}
            <option value={level}>{level}</option>
          {/each}
        </select>
      </label>
      <label>
        Module
        <input bind:value={draft.module} placeholder="koko::web::routes::media" />
      </label>
    </div>
    <div class="form-row log-filter-row">
      <label>
        From
        <input type="datetime-local" bind:value={draft.since} />
      </label>
      <label>
        Until
        <input type="datetime-local" bind:value={draft.until} />
      </label>
    </div>
    <label>
      Search
      <input bind:value={draft.search} placeholder="message text, source path, or module" />
    </label>
    <div class="page-actions">
      <button type="submit">Apply filters</button>
      <button type="button" onclick={clearFilters}>Clear filters</button>
    </div>
  </form>

  {#if error}
    <div class="empty-state">Failed to load logs: {error}</div>
  {:else if loading && entries.length === 0}
    <div class="empty-state tight">Loading…</div>
  {:else if entries.length === 0}
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
            <th class="log-message-col">Message</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as entry (entry.timestamp + entry.message)}
            <tr>
              <td>{entry.timestamp}</td>
              <td><span class="tag {levelTagClass(entry.level)}">{entry.level}</span></td>
              <td>{entry.module}</td>
              <td class="muted">{sourceCell(entry)}</td>
              <td class="log-message-col"><pre class="log-entry-message">{entry.message}</pre></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .section-heading-actions {
    align-items: flex-start;
  }
</style>
