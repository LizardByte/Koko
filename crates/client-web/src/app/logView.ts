/** Compact log results and filtering controls for the settings workspace. */
import { escapeHtml } from './format';
import { state } from './state';
import { renderButtonContent } from './ui';

export const LOG_PAGE_SIZE = 50;

/** Render only the current page of results, preserving the separate filter form. */
export function renderLogResults(): string {
  const entries = state.logsResponse?.entries ?? [];
  const lastPage = Math.max(0, Math.ceil(entries.length / LOG_PAGE_SIZE) - 1);
  const page = Math.min(Math.max(0, state.logPage), lastPage);
  const start = page * LOG_PAGE_SIZE;
  const visible = entries.slice(start, start + LOG_PAGE_SIZE);
  const error = state.logsError ? `<p class="log-error" role="alert">${escapeHtml(state.logsError)}</p>` : '';
  const status = state.logsLoading
    ? 'Loading logs…'
    : entries.length ? `${start + 1}–${start + visible.length} of ${entries.length} recent matches · newest first` : 'No matching entries';
  return `
    ${error}
    <div class="log-results-toolbar">
      <span class="muted" role="status">${status}</span>
      <div class="log-pagination" aria-label="Log pages">
        <button type="button" class="secondary-button icon-only" data-log-page="${page - 1}" ${page === 0 ? 'disabled' : ''} aria-label="Previous log page">${renderButtonContent('', 'chevron-left')}</button>
        <span>${page + 1} / ${lastPage + 1}</span>
        <button type="button" class="secondary-button icon-only" data-log-page="${page + 1}" ${page === lastPage ? 'disabled' : ''} aria-label="Next log page">${renderButtonContent('', 'chevron-right')}</button>
      </div>
    </div>
    <div id="log-events" class="log-events" aria-label="Log entries" aria-busy="${Boolean(state.logsLoading)}">
      ${visible.length ? visible.map((entry) => {
        const levelClass = entry.level === 'ERROR' ? 'danger-tag' : entry.level === 'WARN' ? 'warning' : '';
        const source = `${entry.source_file_path}${typeof entry.line_number === 'number' ? `:${entry.line_number}` : ''}`;
        return `<article class="log-event">
          <div class="log-event-meta">
            <time datetime="${escapeHtml(entry.timestamp)}">${escapeHtml(entry.timestamp)}</time>
            <span class="tag ${levelClass}">${escapeHtml(entry.level)}</span>
            <span class="log-event-module">${escapeHtml(entry.module)}</span>
          </div>
          <pre class="log-entry-message">${escapeHtml(entry.message)}</pre>
          <p class="log-entry-source muted">${escapeHtml(source)}</p>
        </article>`;
      }).join('') : `<div class="empty-state">${state.logsLoading ? 'Loading recent server events…' : 'No entries match these filters. Try a different search or clear the filters.'}</div>`}
    </div>
  `;
}

/** Render filters once; subsequent refreshes update only the result region. */
export function renderLogViewer(): string {
  const disabled = state.logsLoading ? 'disabled' : '';
  return `
    <section class="panel page-panel settings-log-panel">
      <div class="section-heading section-heading-actions">
        <div><h3>Server events</h3><p class="muted">The latest 200 matching entries from your server.</p></div>
        <button type="button" class="secondary-button" id="refresh-log-viewer" ${disabled}>${renderButtonContent('Refresh', 'refresh-cw')}</button>
      </div>
      <form id="log-filter-form" class="settings-form log-filter-form">
        <div class="log-search-row">
          <label>Search<input name="log_search" type="search" value="${escapeHtml(state.logFilters.search)}" placeholder="Search messages, modules or source files" /></label>
          <label>Level<select name="log_level">
            <option value="" ${state.logFilters.level ? '' : 'selected'}>All levels</option>
            ${['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'].map((level) => `<option value="${level}" ${state.logFilters.level === level ? 'selected' : ''}>${level}</option>`).join('')}
          </select></label>
        </div>
        <div class="log-filter-options">
          <label>Module<input name="log_module" value="${escapeHtml(state.logFilters.module)}" placeholder="e.g. koko::media" /></label>
          <label>From<input name="log_since" type="datetime-local" value="${escapeHtml(state.logFilters.since)}" /></label>
          <label>Until<input name="log_until" type="datetime-local" value="${escapeHtml(state.logFilters.until)}" /></label>
        </div>
        <div class="page-actions">
          <button type="submit" ${disabled}>${renderButtonContent('Apply filters', 'search')}</button>
          <button type="button" class="secondary-button" id="clear-log-filters" ${disabled}>${renderButtonContent('Clear', 'x')}</button>
        </div>
      </form>
      <div id="log-results">${renderLogResults()}</div>
      <p class="log-file-path muted">Log file <code>${escapeHtml(state.logsResponse?.log_path ?? 'Unavailable')}</code></p>
    </section>
  `;
}
