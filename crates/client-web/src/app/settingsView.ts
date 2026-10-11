/** Renders settings sections and converts settings forms into API payloads. */
import type { MediaLibrary, MediaLibrarySettings, MetadataProviderSettings, MetadataProviderStatus, ScheduledTaskId, SettingsSnapshot } from '../api';
import { escapeHtml } from './format';
import { formDataString, formDataStrings, joinPaths, normalizedMetadataLanguages, parseBoundedInteger, parsePathsInput } from './formUtils';
import { hasActiveLibraryScan, libraryRefreshProgress } from './activities';
import { renderMetadataDashboard, renderSystemActivitiesPanel } from './dashboardView';
import { renderLogViewer } from './logView';
import { renderUserManagement } from './auth';
import { providerAttributionLogo, providerDisplayName, libraryProviderOptions } from './providers';
import { persistedLibraryForSettings } from './selectors';
import { state } from './state';
import type { AppIconName, SettingsSection } from './types';
import { renderButtonContent, renderIcon, renderPageNavbar } from './ui';

export function activeSettingsSection(): SettingsSection {
  return state.route.page === 'settings' ? state.route.section ?? 'general' : 'general';
}

/** Shared navigation and headings for the settings workspace. */
const SETTINGS_SECTIONS: Array<{ id: SettingsSection; label: string; description: string; icon: AppIconName }> = [
  { id: 'general', label: 'General', description: 'Server, storage and media tools', icon: 'settings' },
  { id: 'libraries', label: 'Libraries', description: 'Media folders, scanning and access', icon: 'layers' },
  { id: 'providers', label: 'Metadata providers', description: 'Credentials and metadata sources', icon: 'link-2' },
  { id: 'scheduled', label: 'Scheduled tasks', description: 'Automated refresh and maintenance', icon: 'refresh-cw' },
  { id: 'users', label: 'Users', description: 'Accounts and permissions', icon: 'user-plus' },
  { id: 'dashboard', label: 'Activity', description: 'Metadata status and background tasks', icon: 'layout-grid' },
  { id: 'logs', label: 'Logs', description: 'Search and inspect server events', icon: 'search' },
];

/** Render section links with an accessible current-page indicator. */
export function renderSettingsSectionNav(): string {
  const activeSection = activeSettingsSection();
  return `
    <nav class="settings-section-nav" aria-label="Settings sections">
      <p class="eyebrow settings-nav-label">Settings</p>
      ${SETTINGS_SECTIONS.map((section) => `
        <a class="settings-nav-link ${activeSection === section.id ? 'active' : ''}" href="/settings${section.id === 'general' ? '' : `/${section.id}`}" ${activeSection === section.id ? 'aria-current="page"' : ''}>
          ${renderIcon(section.icon, 'settings-nav-icon')}
          <span><strong>${escapeHtml(section.label)}</strong><small>${escapeHtml(section.description)}</small></span>
        </a>
      `).join('')}
    </nav>
  `;
}

export const metadataLanguageOptions: Array<{ value: string; label: string }> = [
  { value: 'en-US', label: 'English (United States)' },
  { value: 'en-GB', label: 'English (United Kingdom)' },
  { value: 'es-ES', label: 'Spanish (Spain)' },
  { value: 'fr-FR', label: 'French (France)' },
  { value: 'de-DE', label: 'German (Germany)' },
  { value: 'it-IT', label: 'Italian (Italy)' },
  { value: 'ja-JP', label: 'Japanese (Japan)' },
  { value: 'pt-BR', label: 'Portuguese (Brazil)' },
];

export function metadataLanguageSelect(name: string, selectedLanguages?: string[]): string {
  const selected = normalizedMetadataLanguages(selectedLanguages);
  return `
    <select name="${name}" multiple size="${Math.min(5, metadataLanguageOptions.length)}">
      ${metadataLanguageOptions.map((option) => `
        <option value="${option.value}" ${selected.includes(option.value) ? 'selected' : ''}>${escapeHtml(option.label)}</option>
      `).join('')}
    </select>
  `;
}

export function metadataLanguageModeSelect(name: string, selectedMode: 'auto' | 'manual' = 'auto'): string {
  return `
    <select name="${name}">
      <option value="auto" ${selectedMode === 'auto' ? 'selected' : ''}>Auto</option>
      <option value="manual" ${selectedMode === 'manual' ? 'selected' : ''}>Manual</option>
    </select>
  `;
}

export function userPermissionSelect(name: string, allowedUserIds?: number[]): string {
  const selected = new Set(allowedUserIds ?? []);
  return `
    <select name="${name}" multiple size="${Math.min(5, Math.max(2, state.users.length))}">
      ${state.users.map((user) => `
        <option value="${user.id}" ${selected.has(user.id) ? 'selected' : ''}>${escapeHtml(user.username)}${user.admin ? ' (admin)' : ''}</option>
      `).join('')}
    </select>
  `;
}

function metadataProviderSortIndex(selectedProviders: string[], providerId: string): number {
  const selectedIndex = selectedProviders.indexOf(providerId);
  return selectedIndex < 0 ? Number.MAX_SAFE_INTEGER : selectedIndex;
}

function metadataProviderRoleOrder(provider: MetadataProviderStatus): number {
  return provider.role === 'primary' ? 0 : 1;
}

function compareMetadataProviderOptions(selectedProviders: string[]): (left: MetadataProviderStatus, right: MetadataProviderStatus) => number {
  return (left, right) => {
    return metadataProviderRoleOrder(left) - metadataProviderRoleOrder(right)
      || metadataProviderSortIndex(selectedProviders, left.id) - metadataProviderSortIndex(selectedProviders, right.id)
      || left.display_name.localeCompare(right.display_name);
  };
}

function renderPrimaryProviderMoveButtons(label: string, isSecondary: boolean): string {
  return isSecondary
    ? ''
    : `
            <button type="button" class="secondary-button icon-only" data-provider-move="up" title="Move up" aria-label="Move ${escapeHtml(label)} up">${renderIcon('chevron-up')}</button>
            <button type="button" class="secondary-button icon-only" data-provider-move="down" title="Move down" aria-label="Move ${escapeHtml(label)} down">${renderIcon('chevron-down')}</button>
          `;
}

function renderMetadataProviderOption(
  prefix: string,
  provider: MetadataProviderStatus,
  selected: Set<string>,
  primaryPriority: number,
): string {
  const providerId = provider.id;
  const label = provider.display_name;
  const isSecondary = provider.role === 'secondary';
  const secondaryAvailable = isSecondary
    ? provider.extends_provider_ids.some((primaryProviderId) => selected.has(primaryProviderId))
    : true;
  const checked = selected.has(providerId) && secondaryAvailable;
  const providerPriorityLabel = isSecondary ? 'Secondary' : `Priority ${primaryPriority}`;

  return `
      <div class="metadata-provider-option" data-provider-option="${providerId}" data-provider-role="${provider.role}" data-extends-provider-ids="${provider.extends_provider_ids.join(',')}">
        <div class="provider-option-main">
          <label class="checkbox-inline">
            <input
              name="${prefix}"
              type="checkbox"
              value="${providerId}"
              data-provider-kinds="${provider.supported_kinds.join(',')}"
              ${checked ? 'checked' : ''}
              ${secondaryAvailable ? '' : 'disabled'}
            />
            ${escapeHtml(label)}
          </label>
          <span class="muted">${providerPriorityLabel}</span>
        </div>
        <div class="provider-option-actions">
          ${renderPrimaryProviderMoveButtons(label, isSecondary)}
          <button type="button" class="secondary-button" data-provider-settings="${providerId}">${renderButtonContent('Settings', 'settings')}</button>
        </div>
      </div>
  `;
}

export function metadataProviderCheckboxes(prefix: string, selectedProviders: string[], libraryKind?: string): string {
  const providers = libraryProviderOptions(libraryKind)
    .sort(compareMetadataProviderOptions(selectedProviders));
  const selected = new Set(selectedProviders);
  let primaryPriority = 0;

  return `
    <div class="metadata-provider-list" data-provider-list="${prefix}">
      ${providers.map((provider) => {
        primaryPriority += provider.role === 'primary' ? 1 : 0;
        return renderMetadataProviderOption(prefix, provider, selected, primaryPriority);
      }).join('')}
    </div>
    `;
}

function renderPersistedLibraryTags(persistedLibrary: MediaLibrary | undefined, scanPending: boolean): string {
  if (!persistedLibrary) {
    return '';
  }

  const missingFiles = persistedLibrary.missing_files ?? 0;
  const missingItems = persistedLibrary.missing_items ?? 0;
  const hasMissingItems = missingFiles > 0 || missingItems > 0;
  const scanPendingTag = scanPending ? '<span class="tag warning">Scanning catalog</span>' : '';
  const missingItemsTagClass = hasMissingItems ? 'warning' : 'success';
  const missingItemsLabel = hasMissingItems ? `${missingItems} missing items` : 'No missing items';
  const missingFilesLabel = `${missingFiles} missing files`;
  const missingFilesTag = missingFiles > 0
    ? `<span class="tag warning">${escapeHtml(missingFilesLabel)}</span>`
    : '';

  return `<div class="settings-library-tags">
                ${scanPendingTag}
                <span class="tag ${missingItemsTagClass}">${escapeHtml(missingItemsLabel)}</span>
                ${missingFilesTag}
              </div>`;
}

function renderPersistedLibraryActions(persistedLibrary: MediaLibrary | undefined, refreshPending: boolean, scanPending: boolean): string {
  if (!persistedLibrary) {
    return '';
  }

  const hasMissingItems = (persistedLibrary.missing_files ?? 0) > 0 || (persistedLibrary.missing_items ?? 0) > 0;
  const refreshLabel = refreshPending ? 'Refreshing metadata' : 'Refresh metadata';
  const scanButtonDisabled = scanPending ? 'disabled' : '';
  const scanButtonLabel = scanPending ? 'Scanning' : 'Scan now';
  const refreshButtonDisabled = refreshPending ? 'disabled' : '';
  const deleteMissingDisabled = hasMissingItems ? '' : 'disabled';

  return `
                <button type="button" class="secondary-button" data-scan-library-id="${persistedLibrary.id}" ${scanButtonDisabled}>${renderButtonContent(scanButtonLabel, 'refresh-cw')}</button>
                <button type="button" class="secondary-button" data-refresh-library-id="${persistedLibrary.id}" ${refreshButtonDisabled}>${renderButtonContent(refreshLabel, 'refresh-cw')}</button>
                <button type="button" class="secondary-button danger-button" data-delete-missing-library-id="${persistedLibrary.id}" ${deleteMissingDisabled}>${renderButtonContent('Delete missing', 'trash-2')}</button>
              `;
}

function renderExistingLibrarySettingsCard(library: MediaLibrarySettings, index: number): string {
  const persistedLibrary = persistedLibraryForSettings(library);
  const refreshPending = persistedLibrary ? Boolean(libraryRefreshProgress(persistedLibrary)) : false;
  const scanPending = persistedLibrary ? hasActiveLibraryScan(persistedLibrary.id) : hasActiveLibraryScan();
  const persistedLibraryTags = renderPersistedLibraryTags(persistedLibrary, scanPending);
  const persistedLibraryActions = renderPersistedLibraryActions(persistedLibrary, refreshPending, scanPending);

  return `
      <details class="settings-library-card library-settings-disclosure" id="library-settings-${index}">
        <summary>
          <div class="library-settings-summary-copy">
            <strong>${escapeHtml(library.name || `Library ${index + 1}`)}</strong>
            <span class="muted">${escapeHtml(library.kind.replace(/_/g, ' '))} · ${Math.max(1, library.paths.length)} folder${library.paths.length > 1 ? 's' : ''}</span>
            ${persistedLibraryTags}
          </div>
          ${renderIcon('chevron-right', 'settings-disclosure-icon')}
        </summary>
        <div class="library-settings-body">
          <div class="settings-library-actions">
            ${persistedLibraryActions}
            <button type="button" class="secondary-button danger-button" data-remove-library-index="${index}">${renderButtonContent('Remove library', 'trash-2')}</button>
          </div>
        <div class="form-row">
          <label>Name<input name="existing_library_name_${index}" value="${escapeHtml(library.name)}" /></label>
          <label>Type
            <select name="existing_library_kind_${index}">
              ${libraryKindOptions(library.kind)}
            </select>
          </label>
          <label>Scanner
            <select name="existing_library_scanner_${index}">
              ${libraryScannerOptions(library.scanner ?? 'auto')}
            </select>
          </label>
        </div>
        <label>Folders
          <textarea name="existing_library_paths_${index}" rows="4" placeholder="One folder per line">${escapeHtml(joinPaths(library.paths.length ? library.paths : [library.path].filter(Boolean)))}</textarea>
        </label>
        <div class="form-row">
          <label class="checkbox-inline"><input name="existing_library_recursive_${index}" type="checkbox" ${library.recursive ? 'checked' : ''} /> Recursive scan</label>
        </div>
        <div class="form-row">
          <label>Provider language mode
            ${metadataLanguageModeSelect(`existing_library_metadata_language_mode_${index}`, library.metadata_language_mode)}
          </label>
          <label>Manual languages
            ${metadataLanguageSelect(`existing_library_metadata_language_${index}`, library.metadata_languages)}
          </label>
        </div>
        <div class="form-row">
          <label>Library access
            ${userPermissionSelect(`existing_library_allowed_user_${index}`, library.allowed_user_ids)}
          </label>
        </div>
        <fieldset>
          <legend>Metadata sources</legend>
          ${metadataProviderCheckboxes(`existing_library_metadata_provider_${index}`, library.metadata_providers, library.kind)}
        </fieldset>
        </div>
      </details>
    `;
}

export function renderExistingLibrariesSettings(settings: SettingsSnapshot): string {
  if (!settings.media.libraries.length) {
    return '<div class="empty-state tight">No libraries are configured yet.</div>';
  }

  return settings.media.libraries
    .map((library, index) => renderExistingLibrarySettingsCard(library, index))
    .join('');
}

export function scheduledWeekdayLabel(weekday: string): string {
  return weekday.slice(0, 3).toUpperCase();
}

export function renderScheduledTaskRunButton(taskId: ScheduledTaskId): string {
  return `<button type="button" class="secondary-button" data-run-scheduled-task="${taskId}">${renderButtonContent('Run now', 'play')}</button>`;
}

const SCHEDULED_TASK_WEEKDAYS: SettingsSnapshot['scheduled_tasks']['window']['weekdays'] = [
  'monday',
  'tuesday',
  'wednesday',
  'thursday',
  'friday',
  'saturday',
  'sunday',
];

function renderScheduledStatusTag(enabled: boolean, enabledLabel: string, disabledLabel: string, enabledClass = 'success'): string {
  return `<span class="tag ${enabled ? enabledClass : ''}">${enabled ? enabledLabel : disabledLabel}</span>`;
}

function renderScheduledWeekdayToggles(selectedWeekdays: Set<string>): string {
  return `
              <div class="weekday-toggle-row">
                ${SCHEDULED_TASK_WEEKDAYS.map((weekday) => `
                  <label class="checkbox-inline">
                    <input name="scheduled_window_weekday" type="checkbox" value="${weekday}" ${selectedWeekdays.has(weekday) ? 'checked' : ''} />
                    ${scheduledWeekdayLabel(weekday)}
                  </label>
                `).join('')}
              </div>
  `;
}

export function renderScheduledTasksPage(settings: SettingsSnapshot): string {
  const scheduled = settings.scheduled_tasks;
  const selectedWeekdays = new Set(scheduled.window.weekdays);
  const trashCleanupDays = scheduled.trash_cleanup.missing_item_auto_delete_days ?? 30;

  return `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section>
          <div class="settings-library-card">
            <div class="settings-library-header">
              <div>
                <p class="eyebrow">Runner</p>
                <h3>Task window</h3>
              </div>
              ${renderScheduledStatusTag(scheduled.enabled, 'Enabled', 'Disabled')}
            </div>
            <div class="form-row checkbox-row">
              <label><input name="scheduled_tasks_enabled" type="checkbox" ${scheduled.enabled ? 'checked' : ''} /> Enable scheduled task runner</label>
            </div>
            <div class="form-row">
              <label>Start time<input name="scheduled_window_start_time" type="time" value="${escapeHtml(scheduled.window.start_time)}" /></label>
              <label>Stop time<input name="scheduled_window_stop_time" type="time" value="${escapeHtml(scheduled.window.stop_time)}" /></label>
            </div>
            <fieldset>
              <legend>Run days</legend>
              ${renderScheduledWeekdayToggles(selectedWeekdays)}
            </fieldset>
          </div>

          <div class="settings-library-list">
            <section class="settings-library-card">
              <div class="settings-library-header">
                <div>
                  <p class="eyebrow">Task</p>
                  <h3>Metadata refresh</h3>
                </div>
                <div class="settings-library-actions">
                  ${renderScheduledStatusTag(scheduled.metadata_refresh.enabled, 'Scheduled', 'Manual')}
                  ${renderScheduledTaskRunButton('metadata_refresh')}
                </div>
              </div>
              <div class="form-row checkbox-row">
                <label><input name="scheduled_metadata_refresh_enabled" type="checkbox" ${scheduled.metadata_refresh.enabled ? 'checked' : ''} /> Run stale metadata refreshes automatically</label>
              </div>
              <div class="form-row">
                <label>Refresh interval
                  <select name="metadata_refresh_interval_days">
                    <option value="30" ${settings.metadata.refresh_interval_days === 30 ? 'selected' : ''}>Every 30 days</option>
                    <option value="60" ${settings.metadata.refresh_interval_days === 60 ? 'selected' : ''}>Every 60 days</option>
                    <option value="90" ${settings.metadata.refresh_interval_days === 90 ? 'selected' : ''}>Every 90 days</option>
                    <option value="never" ${settings.metadata.refresh_interval_days == null ? 'selected' : ''}>Never</option>
                  </select>
                </label>
              </div>
            </section>

            <section class="settings-library-card">
              <div class="settings-library-header">
                <div>
                  <p class="eyebrow">Task</p>
                  <h3>Trash cleanup</h3>
                </div>
                <div class="settings-library-actions">
                  ${renderScheduledStatusTag(scheduled.trash_cleanup.enabled, 'Scheduled', 'Manual', 'warning')}
                  ${renderScheduledTaskRunButton('trash_cleanup')}
                </div>
              </div>
              <div class="form-row checkbox-row">
                <label><input name="scheduled_trash_cleanup_enabled" type="checkbox" ${scheduled.trash_cleanup.enabled ? 'checked' : ''} /> Delete missing items automatically</label>
              </div>
              <div class="form-row">
                <label>Days missing
                  <input name="scheduled_trash_cleanup_days" type="number" min="1" max="3650" value="${trashCleanupDays}" />
                </label>
                <label>Run interval (days)
                  <input name="scheduled_trash_cleanup_interval_days" type="number" min="1" max="365" value="${scheduled.trash_cleanup.interval_days}" />
                </label>
              </div>
            </section>

            <section class="settings-library-card">
              <div class="settings-library-header">
                <div>
                  <p class="eyebrow">Task</p>
                  <h3>Database maintenance</h3>
                </div>
                <div class="settings-library-actions">
                  ${renderScheduledStatusTag(scheduled.database_maintenance.enabled, 'Scheduled', 'Manual')}
                  ${renderScheduledTaskRunButton('database_maintenance')}
                </div>
              </div>
              <div class="form-row checkbox-row">
                <label><input name="scheduled_database_maintenance_enabled" type="checkbox" ${scheduled.database_maintenance.enabled ? 'checked' : ''} /> Checkpoint, vacuum, and optimize automatically</label>
              </div>
              <div class="form-row">
                <label>Run interval (days)
                  <input name="scheduled_database_maintenance_interval_days" type="number" min="1" max="365" value="${scheduled.database_maintenance.interval_days}" />
                </label>
              </div>
            </section>
          </div>
        </section>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save scheduled tasks', 'save')}</button>
        </div>
      </form>
    </section>
  `;
}

export function libraryKindOptions(selectedKind: string): string {
  return [
    ['movies', 'Movies'],
    ['shows', 'Shows'],
    ['music', 'Music'],
    ['photos', 'Photos'],
    ['books', 'Books'],
    ['home_videos', 'Home videos'],
  ]
    .map(([value, label]) => `<option value="${value}" ${selectedKind === value ? 'selected' : ''}>${label}</option>`)
    .join('');
}

export function libraryScannerOptions(selectedScanner: string): string {
  return [
    ['auto', 'Auto'],
    ['directory', 'Directory'],
    ['movies', 'Movies'],
    ['shows', 'Shows'],
    ['music', 'Music'],
    ['photos', 'Photos'],
    ['books', 'Books'],
  ]
    .map(([value, label]) => `<option value="${value}" ${selectedScanner === value ? 'selected' : ''}>${label}</option>`)
    .join('');
}

export function renderProviderSettingsCard(provider: MetadataProviderSettings): string {
  const label = providerDisplayName(provider.id);
  const status = state.metadataProviders.find((entry) => entry.id === provider.id);
  const logoUrl = providerAttributionLogo(provider.id);
  const showApiKey = Boolean(status?.requires_api_key);
  const apiKeyConfigured = Boolean(provider.api_key_configured || provider.api_key_secret_ref || provider.api_key);
  const logoMarkup = logoUrl ? `<span class="provider-settings-logo" aria-hidden="true">
    ${renderIcon('link-2', 'provider-logo-fallback')}
    <img data-provider-logo src="${escapeHtml(logoUrl)}" alt="" />
  </span>` : '';
  const providerRoleLabel = status?.role === 'secondary' ? 'Secondary' : 'Primary';
  const providerRoleTag = status?.role ? `<span class="tag">${escapeHtml(providerRoleLabel)}</span>` : '';
  const providerDescription = status?.description ? `<p class="muted">${escapeHtml(status.description)}</p>` : '';
  const providerAttribution = status?.attribution_text ? `<p class="muted">${escapeHtml(status.attribution_text)}</p>` : '';
  const apiKeyId = `${provider.id}-api-key`;
  const apiKeyPlaceholder = apiKeyConfigured ? 'Saved' : '';
  const apiKeyField = showApiKey
    ? `<div class="api-key-field">
        <label for="${apiKeyId}">API key</label>
        <div class="input-group api-key-picker" role="group" aria-label="${escapeHtml(label)} API key" data-api-key-configured="${apiKeyConfigured}">
          <input id="${apiKeyId}" class="input-group-field" name="${provider.id}_api_key" type="password" value="" placeholder="${apiKeyPlaceholder}" autocomplete="new-password" data-api-key-draft />
          <input name="${provider.id}_clear_api_key" type="checkbox" data-api-key-removal hidden />
          <button type="button" class="secondary-button icon-only" data-clear-api-key title="Clear API key on save" aria-label="Clear ${escapeHtml(label)} API key" aria-pressed="false" ${apiKeyConfigured ? '' : 'disabled'}>${renderIcon('x')}</button>
        </div>
        <small class="muted" data-api-key-status aria-live="polite" hidden>Saved API key will be cleared on save.</small>
      </div>`
    : '';
  const providerSettingsFields = showApiKey
    ? `<div class="form-row provider-credentials">
        ${apiKeyField}
      </div>`
    : '<p class="muted">This provider does not require provider-specific settings.</p>';
  return `
    <section class="settings-library-card provider-settings-card" id="provider-${escapeHtml(provider.id)}">
      <div class="settings-library-header">
        <div class="provider-settings-title">
          ${logoMarkup}
          <div>
          <p class="eyebrow">Provider</p>
          <h3>${escapeHtml(label)}</h3>
          </div>
        </div>
        ${providerRoleTag}
      </div>
      ${providerDescription}
      ${providerAttribution}
      ${providerSettingsFields}
    </section>
  `;
}

export function renderProviderSettingsPage(settings: SettingsSnapshot): string {
  return `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section>
          <p class="muted">Manage provider credentials here. Metadata languages are selected per library.</p>
          <div class="settings-library-list">
            ${settings.metadata.providers.map(renderProviderSettingsCard).join('')}
          </div>
          <div class="form-row">
            <button type="button" class="secondary-button" id="clear-metadata-cache">${renderButtonContent('Clear metadata cache', 'trash-2')}</button>
            <p class="muted">Provider response cache is kept for 24 hours by default.</p>
          </div>
        </section>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save provider settings', 'save')}</button>
        </div>
      </form>
    </section>
  `;
}

function renderGeneralSettingsPage(settings: SettingsSnapshot): string {
  const useHttpsChecked = settings.server.use_https ? 'checked' : '';
  const useCustomCertsChecked = settings.server.use_custom_certs ? 'checked' : '';
  return `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section class="settings-group">
          <div class="settings-group-heading"><h3>Connection</h3><p class="muted">Choose how devices connect to your server.</p></div>
          <div class="form-row">
            <label>Listen address<input name="address" value="${escapeHtml(settings.server.address)}" /><small class="muted">Use 0.0.0.0 to listen on all network interfaces.</small></label>
            <label>Port<input name="port" type="number" min="1" max="65535" value="${settings.server.port}" /></label>
          </div>
          <div class="form-row checkbox-row">
            <label class="checkbox-inline"><input name="use_https" type="checkbox" ${useHttpsChecked} /> Use HTTPS</label>
            <label class="checkbox-inline"><input name="use_custom_certs" type="checkbox" ${useCustomCertsChecked} /> Use custom certificates</label>
          </div>
          <div class="form-row">
            <label>Certificate path<input name="cert_path" value="${escapeHtml(settings.server.cert_path)}" /></label>
            <label>Key path<input name="key_path" value="${escapeHtml(settings.server.key_path)}" /></label>
          </div>
        </section>

        <section class="settings-group">
          <div class="settings-group-heading"><h3>Storage</h3><p class="muted">Where Koko keeps its database and cached metadata.</p></div>
          <label>Data directory<input name="data_dir" value="${escapeHtml(settings.general.data_dir)}" /></label>
        </section>

        <section class="settings-group">
          <div class="settings-group-heading"><h3>Media tools</h3><p class="muted">Leave a path blank to use the bundled tool.</p></div>
          <div class="form-row">
            <label>ffmpeg path<input name="ffmpeg_path" value="${escapeHtml(settings.ffmpeg.ffmpeg_path)}" /></label>
            <label>ffprobe path<input name="ffprobe_path" value="${escapeHtml(settings.ffmpeg.ffprobe_path)}" /></label>
          </div>
        </section>

        <div class="page-actions settings-save-bar">
          <button type="submit">${renderButtonContent('Save settings', 'save')}</button>
        </div>
      </form>
    </section>
  `;
}

function renderLibrarySettingsPage(settings: SettingsSnapshot): string {
  return `
    <section class="panel page-panel settings-page-panel">
      <form id="settings-form" class="settings-form">
        <section>
          <p class="muted">Expand a library to edit its folders, scanning and metadata preferences.</p>
          <div class="settings-library-list">
            ${renderExistingLibrariesSettings(settings)}
          </div>
        </section>
        <div class="page-actions">
          <button type="submit">${renderButtonContent('Save library settings', 'save')}</button>
        </div>
      </form>

      <details class="settings-add-library">
        <summary>${renderButtonContent('Add a library', 'plus')}</summary>
        <form id="add-library-form" class="settings-form add-library-form">
        <section>
          <h3>Add library</h3>
          <label>Name<input name="library_name" placeholder="Movies" required /></label>
          <label>Folders
            <textarea name="library_paths" rows="4" placeholder="C:/Media/Movies&#10;D:/Overflow/Movies" required></textarea>
          </label>
          <div class="form-row">
            <label>Type
              <select name="library_kind">
                ${libraryKindOptions('movies')}
              </select>
            </label>
            <label>Scanner
              <select name="library_scanner">
                ${libraryScannerOptions('auto')}
              </select>
            </label>
            <label class="checkbox-inline"><input name="library_recursive" type="checkbox" checked /> Recursive scan</label>
          </div>
          <div class="form-row">
            <label>Provider language mode
              ${metadataLanguageModeSelect('library_metadata_language_mode', 'auto')}
            </label>
            <label>Manual languages
              ${metadataLanguageSelect('library_metadata_language', ['en-US'])}
            </label>
          </div>
          <div class="form-row">
            <label>Library access
              ${userPermissionSelect('library_allowed_user', [])}
            </label>
          </div>
          <fieldset>
            <legend>Metadata sources</legend>
            <div id="add-library-metadata-providers">${metadataProviderCheckboxes('library_metadata_provider', ['tmdb'])}</div>
          </fieldset>
        </section>
        <button type="submit">${renderButtonContent('Add library', 'plus')}</button>
      </form>
      </details>
    </section>
  `;
}

function renderSettingsSectionContent(section: SettingsSection, settings: SettingsSnapshot): string {
  if (section === 'general') {
    return renderGeneralSettingsPage(settings);
  }
  if (section === 'providers') {
    return renderProviderSettingsPage(settings);
  }
  if (section === 'scheduled') {
    return renderScheduledTasksPage(settings);
  }
  if (section === 'libraries') {
    return renderLibrarySettingsPage(settings);
  }
  if (section === 'users') {
    return `<section class="panel page-panel settings-page-panel">${renderUserManagement()}</section>`;
  }
  if (section === 'dashboard') {
    return `
      <div id="metadata-dashboard-panel-root">${renderMetadataDashboard()}</div>
      <div id="system-activities-panel-root">${renderSystemActivitiesPanel()}</div>
    `;
  }
  if (section === 'logs') {
    return '<div id="log-viewer-panel-root">' + renderLogViewer() + '</div>';
  }
  return '';
}

export function renderSettingsPage(): string {
  const settings = state.settingsResponse?.settings;
  if (!settings) {
    return '<section class="panel page-panel"><div class="empty-state">Settings are still loading…</div></section>';
  }

  const section = activeSettingsSection();
  const settingsContent = renderSettingsSectionContent(section, settings);
  const heading = SETTINGS_SECTIONS.find((entry) => entry.id === section)!;

  return `
    <div class="settings-workspace">
      ${renderSettingsSectionNav()}
      <div class="settings-content">
        ${renderPageNavbar('Settings', heading.label, heading.description)}
        ${settingsContent}
      </div>
    </div>
  `;
}

export function buildSettingsFromForm(formData: FormData): SettingsSnapshot | undefined {
  const current = state.settingsResponse?.settings;
  if (!current) {
    return undefined;
  }
  const settingsSection = activeSettingsSection();
  let metadataRefreshIntervalDays = current.metadata.refresh_interval_days;
  if (formData.has('metadata_refresh_interval_days')) {
    const refreshIntervalValue = formDataString(formData.get('metadata_refresh_interval_days'));
    if (refreshIntervalValue === 'never') {
      metadataRefreshIntervalDays = null;
    } else {
      metadataRefreshIntervalDays = Number(formDataString(
        formData.get('metadata_refresh_interval_days'),
        String(current.metadata.refresh_interval_days ?? 30),
      ));
    }
  }

  return {
    general: {
      data_dir: formDataString(formData.get('data_dir'), current.general.data_dir),
    },
    media: {
      missing_item_auto_delete_days: null,
      libraries: current.media.libraries.map((library, index) => {
        const pathsField = `existing_library_paths_${index}`;
        if (!formData.has(pathsField)) {
          return library;
        }

        const paths = parsePathsInput(formData.get(pathsField));
        const providerField = `existing_library_metadata_provider_${index}`;
        return {
          name: formDataString(formData.get(`existing_library_name_${index}`), library.name),
          path: paths[0] ?? library.path,
          paths,
          recursive: formData.get(`existing_library_recursive_${index}`) === 'on',
          kind: formDataString(formData.get(`existing_library_kind_${index}`), library.kind),
          scanner: formDataString(formData.get(`existing_library_scanner_${index}`), library.scanner ?? 'auto'),
          metadata_providers: formDataStrings(formData.getAll(providerField)),
          metadata_language_mode: formDataString(formData.get(`existing_library_metadata_language_mode_${index}`), library.metadata_language_mode ?? 'auto') === 'manual'
            ? 'manual'
            : 'auto',
          metadata_languages: formData.has(`existing_library_metadata_language_${index}`)
            ? normalizedMetadataLanguages(formDataStrings(formData.getAll(`existing_library_metadata_language_${index}`)))
            : normalizedMetadataLanguages(library.metadata_languages),
          allowed_user_ids: formData.has(`existing_library_allowed_user_${index}`)
            ? formData.getAll(`existing_library_allowed_user_${index}`)
                .map(Number)
                .filter((value) => Number.isFinite(value) && value > 0)
            : library.allowed_user_ids,
        };
      }),
    },
    metadata: {
      refresh_interval_days: metadataRefreshIntervalDays,
      providers: current.metadata.providers.map((provider) => {
        const prefix = provider.id;
        if (
          !formData.has(`${prefix}_api_key`)
          && !formData.has(`${prefix}_clear_api_key`)
        ) {
          return provider;
        }

        const submittedApiKey = formData.has(`${prefix}_api_key`)
          ? formDataString(formData.get(`${prefix}_api_key`)).trim()
          : undefined;
        const clearApiKey = formData.get(`${prefix}_clear_api_key`) === 'on';

        return {
          ...provider,
          api_key: submittedApiKey && !clearApiKey ? submittedApiKey : null,
          clear_api_key: clearApiKey,
        };
      }),
    },
    server: {
      use_https: settingsSection === 'general' ? formData.get('use_https') === 'on' : current.server.use_https,
      address: formDataString(formData.get('address'), current.server.address),
      port: Number(formData.get('port') ?? current.server.port),
      cert_path: formDataString(formData.get('cert_path'), current.server.cert_path),
      key_path: formDataString(formData.get('key_path'), current.server.key_path),
      use_custom_certs: settingsSection === 'general'
        ? formData.get('use_custom_certs') === 'on'
        : current.server.use_custom_certs,
    },
    ffmpeg: {
      ffmpeg_path: formDataString(formData.get('ffmpeg_path'), current.ffmpeg.ffmpeg_path),
      ffprobe_path: formDataString(formData.get('ffprobe_path'), current.ffmpeg.ffprobe_path),
    },
    scheduled_tasks: parseScheduledTasksSettings(formData, current),
  };
}

export function parseScheduledTasksSettings(formData: FormData, current: SettingsSnapshot): SettingsSnapshot['scheduled_tasks'] {
  if (!formData.has('scheduled_window_start_time')) {
    return current.scheduled_tasks;
  }

  const weekdays = formData.getAll('scheduled_window_weekday')
    .filter((value): value is string => typeof value === 'string')
    .filter((value): value is SettingsSnapshot['scheduled_tasks']['window']['weekdays'][number] => (
      ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'].includes(value)
    ));

  return {
    enabled: formData.get('scheduled_tasks_enabled') === 'on',
    window: {
      start_time: formDataString(formData.get('scheduled_window_start_time'), current.scheduled_tasks.window.start_time),
      stop_time: formDataString(formData.get('scheduled_window_stop_time'), current.scheduled_tasks.window.stop_time),
      weekdays: weekdays.length ? weekdays : current.scheduled_tasks.window.weekdays,
    },
    metadata_refresh: {
      enabled: formData.get('scheduled_metadata_refresh_enabled') === 'on',
    },
    trash_cleanup: {
      enabled: formData.get('scheduled_trash_cleanup_enabled') === 'on',
      missing_item_auto_delete_days: parseBoundedInteger(
        formData.get('scheduled_trash_cleanup_days'),
        current.scheduled_tasks.trash_cleanup.missing_item_auto_delete_days ?? 30,
        1,
        3650,
      ),
      interval_days: parseBoundedInteger(
        formData.get('scheduled_trash_cleanup_interval_days'),
        current.scheduled_tasks.trash_cleanup.interval_days,
        1,
        365,
      ),
    },
    database_maintenance: {
      enabled: formData.get('scheduled_database_maintenance_enabled') === 'on',
      interval_days: parseBoundedInteger(
        formData.get('scheduled_database_maintenance_interval_days'),
        current.scheduled_tasks.database_maintenance.interval_days,
        1,
        365,
      ),
    },
  };
}
