/** Renders metadata dashboard, system activity, and log viewer panels. */
import type { MediaItemSummary } from '../api';
import { escapeHtml, formatTimestamp } from './format';
import { activityProgress, itemHasActiveMetadataRefresh, itemIsMetadataPending } from './activities';
import { state } from './state';
import { formatChildCount, humanizeItemType, renderButtonContent } from './ui';

export function metadataDashboardRefreshState(item: MediaItemSummary): 'pending' | 'stalled' | 'error' | 'fresh' | 'unmatched' {
  if (itemIsMetadataPending(item)) {
    return itemHasActiveMetadataRefresh(item) ? 'pending' : 'stalled';
  }

  if (item.metadata_refresh_state === 'error') {
    return 'error';
  }

  if (item.metadata_refresh_state === 'fresh' || item.has_metadata) {
    return 'fresh';
  }

  return 'unmatched';
}

export function metadataDashboardRefreshLabel(item: MediaItemSummary): string {
  switch (metadataDashboardRefreshState(item)) {
    case 'pending':
      return 'Refreshing';
    case 'stalled':
      return 'Pending without worker';
    case 'error':
      return 'Failed';
    case 'fresh':
      return 'Up to date';
    default:
      return 'Not linked';
  }
}

export function filteredMetadataDashboardItems(): MediaItemSummary[] {
  const libraryFilter = state.metadataDashboardFilters.libraryId;
  const itemTypeFilter = state.metadataDashboardFilters.itemType;
  const refreshStateFilter = state.metadataDashboardFilters.refreshState;
  const searchFilter = state.metadataDashboardFilters.search.trim().toLowerCase();

  const rank = (item: MediaItemSummary): number => {
    switch (metadataDashboardRefreshState(item)) {
      case 'error':
        return 0;
      case 'stalled':
        return 1;
      case 'pending':
        return 2;
      case 'unmatched':
        return 3;
      default:
        return 4;
    }
  };

  return [...state.dashboardItems]
    .filter((item) => {
      const matchesLibrary = libraryFilter ? String(item.library_id) === libraryFilter : true;
      const matchesItemType = itemTypeFilter ? item.item_type === itemTypeFilter : true;
      const matchesRefreshState = refreshStateFilter ? metadataDashboardRefreshState(item) === refreshStateFilter : true;
      const matchesSearch = searchFilter
        ? `${item.display_title} ${item.relative_path} ${item.metadata_refresh_error ?? ''}`.toLowerCase().includes(searchFilter)
        : true;
      return matchesLibrary && matchesItemType && matchesRefreshState && matchesSearch;
    })
    .sort((left, right) => {
      return rank(left) - rank(right)
        || left.library_id - right.library_id
        || left.display_title.localeCompare(right.display_title)
        || left.relative_path.localeCompare(right.relative_path);
    });
}

export function metadataDashboardSummary(items: MediaItemSummary[]): {
  failed: number;
  pending: number;
  stalled: number;
  unmatched: number;
} {
  return items.reduce((summary, item) => {
    switch (metadataDashboardRefreshState(item)) {
      case 'error':
        summary.failed += 1;
        break;
      case 'pending':
        summary.pending += 1;
        break;
      case 'stalled':
        summary.stalled += 1;
        break;
      case 'unmatched':
        summary.unmatched += 1;
        break;
      default:
        break;
    }
    return summary;
  }, {
    failed: 0,
    pending: 0,
    stalled: 0,
    unmatched: 0,
  });
}

export function renderMetadataDashboard(): string {
  const filteredItems = filteredMetadataDashboardItems();
  const summary = metadataDashboardSummary(state.dashboardItems);
  const itemTypes = [...new Set(state.dashboardItems.map((item) => item.item_type))].sort((left, right) => left.localeCompare(right));
  let dashboardContent = '<div class="empty-state tight">No items matched the current dashboard filters.</div>';
  if (filteredItems.length) {
    dashboardContent = `<div class="table-shell metadata-dashboard-table-shell">
            <table class="data-table metadata-dashboard-table">
              <thead>
                <tr>
                  <th>Title</th>
                  <th>Type</th>
                  <th>Library</th>
                  <th>Refresh state</th>
                  <th>Artwork updated</th>
                  <th>Children</th>
                  <th></th>
                </tr>
              </thead>
          <tbody>${filteredItems.map((item) => {
            const library = state.libraries.find((entry) => entry.id === item.library_id);
            const refreshState = metadataDashboardRefreshState(item);
            let refreshStateTagClass = '';
            if (refreshState === 'error') {
              refreshStateTagClass = 'danger-tag';
            } else if (refreshState === 'pending' || refreshState === 'stalled') {
              refreshStateTagClass = 'warning';
            } else if (refreshState === 'fresh') {
              refreshStateTagClass = 'success';
            }
            return `
              <tr>
                <td>
                  <div class="table-title-cell">
                    <strong>${escapeHtml(item.display_title)}</strong>
                    <p class="muted metadata-dashboard-path">${escapeHtml(item.relative_path)}</p>
                    ${item.metadata_refresh_error ? `<p class="metadata-dashboard-error">${escapeHtml(item.metadata_refresh_error)}</p>` : ''}
                  </div>
                </td>
                <td>${escapeHtml(humanizeItemType(item.item_type))}</td>
                <td>${escapeHtml(library?.name ?? `Library ${item.library_id}`)}</td>
                <td><span class="tag ${refreshStateTagClass}">${escapeHtml(metadataDashboardRefreshLabel(item))}</span></td>
                <td>${escapeHtml(formatTimestamp(item.artwork_updated_at))}</td>
                <td>${escapeHtml(formatChildCount(item))}</td>
                <td><button type="button" class="secondary-button" data-item-id="${item.id}">${renderButtonContent('Open item', 'arrow-left', 'end')}</button></td>
              </tr>
            `;
          }).join('')}</tbody>
            </table>
          </div>`;
  }

  return `
    <section class="panel page-panel metadata-dashboard-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Metadata dashboard</h3>
          <p class="muted">Browse every item, identify failed refreshes, and spot pending items that no longer have an active worker.</p>
        </div>
        <div class="provider-tags">
          <span class="tag">${state.dashboardItems.length} total</span>
          <span class="tag ${summary.failed ? 'danger-tag' : ''}">${summary.failed} failed</span>
          <span class="tag warning">${summary.pending} active</span>
          <span class="tag ${summary.stalled ? 'warning' : ''}">${summary.stalled} stalled</span>
          <span class="tag">${summary.unmatched} unmatched</span>
        </div>
      </div>
      <form id="metadata-dashboard-filter-form" class="settings-form metadata-dashboard-filter-form">
        <div class="form-row metadata-dashboard-filter-grid">
          <label>Library
            <select name="dashboard_library_id">
              <option value="" ${state.metadataDashboardFilters.libraryId ? '' : 'selected'}>All libraries</option>
              ${state.libraries.map((library) => `<option value="${library.id}" ${state.metadataDashboardFilters.libraryId === String(library.id) ? 'selected' : ''}>${escapeHtml(library.name)}</option>`).join('')}
            </select>
          </label>
          <label>Item type
            <select name="dashboard_item_type">
              <option value="" ${state.metadataDashboardFilters.itemType ? '' : 'selected'}>All item types</option>
              ${itemTypes.map((itemType) => `<option value="${escapeHtml(itemType)}" ${state.metadataDashboardFilters.itemType === itemType ? 'selected' : ''}>${escapeHtml(humanizeItemType(itemType))}</option>`).join('')}
            </select>
          </label>
          <label>Refresh state
            <select name="dashboard_refresh_state">
              <option value="" ${state.metadataDashboardFilters.refreshState ? '' : 'selected'}>All states</option>
              ${[
                ['error', 'Failed'],
                ['stalled', 'Pending without worker'],
                ['pending', 'Refreshing'],
                ['fresh', 'Up to date'],
                ['unmatched', 'Not linked'],
              ].map(([value, label]) => `<option value="${value}" ${state.metadataDashboardFilters.refreshState === value ? 'selected' : ''}>${label}</option>`).join('')}
            </select>
          </label>
        </div>
        <label>Search
          <input name="dashboard_search" value="${escapeHtml(state.metadataDashboardFilters.search)}" placeholder="Title, path, or refresh error" />
        </label>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Apply filters', 'search')}</button>
          <button type="button" class="secondary-button" id="clear-metadata-dashboard-filters">${renderButtonContent('Clear filters', 'x')}</button>
        </div>
      </form>
      ${dashboardContent}
    </section>
  `;
}

export function renderSystemActivitiesPanel(): string {
  const activities = state.systemActivities.filter((activity) => activity.state !== 'completed' && activity.state !== 'failed');
  let activitiesContent = '<div class="empty-state tight">No background activities are running right now.</div>';
  if (activities.length) {
    activitiesContent = `<div class="settings-system-activity-list">${activities.map((activity) => {
            const progress = activityProgress(activity);
            return `
              <article class="settings-system-activity">
                <div class="settings-system-activity-header">
                  <div>
                    <strong>${escapeHtml(activity.label)}</strong>
                    <p class="muted">${escapeHtml(activity.scope)} · ${escapeHtml(activity.source)}</p>
                  </div>
                  <div class="provider-tags">
                    <span class="tag ${activity.state === 'running' ? 'warning' : ''}">${escapeHtml(activity.state)}</span>
                    ${activity.provider_id ? `<span class="tag">${escapeHtml(activity.provider_id)}</span>` : ''}
                  </div>
                </div>
                <div class="activity-progress-row">
                  <div class="activity-progress-bar" aria-hidden="true">
                    <span class="activity-progress-fill" style="--activity-progress: ${progress.percent}%;"></span>
                  </div>
                  <span class="muted">${progress.completed}/${progress.total}${progress.failed ? ` · ${progress.failed} failed` : ''}</span>
                </div>
              </article>
            `;
          }).join('')}</div>`;
  }

  return `
    <section class="panel page-panel settings-activity-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Backend activities</h3>
          <p class="muted">Active background work that the browser is polling.</p>
        </div>
        <span class="tag">${activities.length} active</span>
      </div>
      ${activitiesContent}
    </section>
  `;
}

export function renderLogViewer(): string {
  const logEntries = state.logsResponse?.entries ?? [];
  let logEntriesContent = '<div class="empty-state tight">No log entries matched the current filters.</div>';
  if (logEntries.length) {
    logEntriesContent = `<div class="table-shell">
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
              <tbody>${logEntries.map((entry) => {
                let levelTagClass = '';
                if (entry.level === 'ERROR') {
                  levelTagClass = 'danger-tag';
                } else if (entry.level === 'WARN') {
                  levelTagClass = 'warning';
                }
                return `
                <tr>
                  <td>${escapeHtml(entry.timestamp)}</td>
                  <td><span class="tag ${levelTagClass}">${escapeHtml(entry.level)}</span></td>
                  <td>${escapeHtml(entry.module)}</td>
                  <td class="muted">${escapeHtml(entry.source_file_path)}${typeof entry.line_number === 'number' ? `:${entry.line_number}` : ''}</td>
                  <td class="log-message-col"><pre class="log-entry-message">${escapeHtml(entry.message)}</pre></td>
                </tr>
              `;
              }).join('')}</tbody>
            </table>
          </div>`;
  }

  return `
    <section class="panel page-panel settings-log-panel">
      <div class="section-heading section-heading-actions">
        <div>
          <h3>Logs</h3>
          <p class="muted">Structured logs from ${escapeHtml(state.logsResponse?.log_path ?? 'the current log file')}.</p>
        </div>
        <button type="button" class="secondary-button" id="refresh-log-viewer">${renderButtonContent('Refresh logs', 'refresh-cw')}</button>
      </div>
      <form id="log-filter-form" class="settings-form log-filter-form">
        <div class="form-row log-filter-row">
          <label>Level
            <select name="log_level">
              <option value="" ${state.logFilters.level ? '' : 'selected'}>All levels</option>
              ${['TRACE', 'DEBUG', 'INFO', 'WARN', 'ERROR'].map((level) => `<option value="${level}" ${state.logFilters.level === level ? 'selected' : ''}>${level}</option>`).join('')}
            </select>
          </label>
          <label>Module<input name="log_module" value="${escapeHtml(state.logFilters.module)}" placeholder="koko::web::routes::media" /></label>
        </div>
        <div class="form-row log-filter-row">
          <label>From
            <input name="log_since" type="datetime-local" value="${escapeHtml(state.logFilters.since)}" />
          </label>
          <label>Until
            <input name="log_until" type="datetime-local" value="${escapeHtml(state.logFilters.until)}" />
          </label>
        </div>
        <label>Search<input name="log_search" value="${escapeHtml(state.logFilters.search)}" placeholder="message text, source path, or module" /></label>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Apply filters', 'search')}</button>
          <button type="button" class="secondary-button" id="clear-log-filters">${renderButtonContent('Clear filters', 'x')}</button>
        </div>
      </form>
      ${logEntriesContent}
    </section>
  `;
}
