import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { createServer as createHttpServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

// Load the actual TypeScript views through the existing Vite dependency.
globalThis.location = { pathname: '/settings', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({
  root: fileURLToPath(new URL('..', import.meta.url)),
  server: { middlewareMode: true, hmr: false, ws: false },
});
after(() => server.close());
const assetServer = createHttpServer(server.middlewares);
await new Promise((resolve) => assetServer.listen(0, '127.0.0.1', resolve));
after(() => new Promise((resolve) => assetServer.close(resolve)));
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderLogResults, renderLogViewer } = await server.ssrLoadModule('/src/app/logView.ts');
const { renderSettingsPage, buildSettingsFromForm } = await server.ssrLoadModule('/src/app/settingsView.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { getMockSettings, getMockMetadataProviders, updateMockSettings } = await server.ssrLoadModule('/src/mockApi.ts');
const { providerAttributionLogo } = await server.ssrLoadModule('/src/app/providers.ts');
const { escapeHtml } = await server.ssrLoadModule('/src/app/format.ts');
const { renderButtonContent } = await server.ssrLoadModule('/src/app/ui.ts');

beforeEach(() => {
  globalThis.location.pathname = '/settings';
  state.route = parseRoute();
  state.settingsResponse = getMockSettings();
  state.metadataProviders = getMockMetadataProviders();
  state.bootstrap = { current_user: { id: 1, username: 'admin', admin: true } };
  state.users = [state.bootstrap.current_user];
  state.isLoading = false;
  state.logsLoading = false;
  state.logsError = undefined;
  state.logPage = 0;
  state.logsResponse = {
    log_path: '/tmp/koko.log',
    entries: Array.from({ length: 123 }, (_, index) => ({
      timestamp: '2026-10-11T10:00:00Z',
      level: 'INFO',
      module: 'koko',
      source_file_path: 'src/main.rs',
      line_number: index,
      message: `event-${index}`,
    })),
  };
});

test('log pages bound the rendered entries and retain every result in order', () => {
  const messages = [];
  for (let page = 0; page < 3; page += 1) {
    state.logPage = page;
    const html = renderLogResults();
    const matches = [...html.matchAll(/<pre class="log-entry-message">event-(\d+)<\/pre>/g)];
    assert.equal(matches.length, page < 2 ? 50 : 23);
    messages.push(...matches.map((match) => Number(match[1])));
  }
  assert.deepEqual(messages, Array.from({ length: 123 }, (_, index) => index));
  state.logPage = 99;
  assert.match(renderLogResults(), /101–123 of 123/);
  assert.match(renderLogResults(), /data-log-page="3" disabled/);
});

test('log content and inline errors are escaped and empty results stay usable', () => {
  state.logsResponse.entries[0].message = '<script>alert("test")</script>';
  state.logsError = '<img src=x onerror=alert(1)>';
  const html = renderLogResults();
  assert.ok(!html.includes('<script>'));
  assert.ok(!html.includes('<img'));
  assert.match(html, /&lt;script&gt;/);
  assert.match(html, /role="alert"/);
  state.logsResponse.entries = [];
  assert.match(renderLogResults(), /No entries match these filters/);
});

test('loading logs communicates progress and disables duplicate request controls', () => {
  state.logsLoading = true;
  const html = renderLogViewer();
  assert.match(html, /Loading logs/);
  assert.match(html, /aria-busy="true"/);
  assert.match(html, /id="refresh-log-viewer" disabled/);
  assert.match(html, /type="submit" disabled/);
  assert.match(html, /id="clear-log-filters" disabled/);
});

test('opening logs keeps the settings layout and shows progress before settings or logs arrive', () => {
  state.route = { page: 'settings', section: 'logs' };
  state.settingsResponse = undefined;
  state.logsResponse = undefined;
  state.isLoading = true;
  const html = renderSettingsPage();
  assert.match(html, /class="settings-workspace"/);
  assert.match(html, /href="\/settings\/logs" aria-current="page"/);
  assert.match(html, /<h2>Logs<\/h2>/);
  assert.match(html, /id="log-filter-form"/);
  assert.match(html, /class="log-results-placeholder">Loading recent server events…/);
  assert.match(html, /aria-busy="true"/);
  assert.match(html, /id="refresh-log-viewer" disabled/);
  assert.doesNotMatch(html, /No matching entries|No entries match|Unavailable|Settings are still loading/);
  state.isLoading = false;
  const loaded = renderSettingsPage();
  assert.match(loaded, /No entries match these filters/);
  assert.match(loaded, /aria-busy="false"/);
  assert.doesNotMatch(loaded, /id="refresh-log-viewer" disabled/);
});

test('settings navigation and heading remain present while configuration loads', () => {
  state.settingsResponse = undefined;
  const html = renderSettingsPage();
  assert.match(html, /class="settings-workspace"/);
  assert.match(html, /<h2>General<\/h2>/);
  assert.match(html, /role="status">Settings are still loading/);
});

test('route loading retains cached log entries while disabling pagination until refreshed', () => {
  state.isLoading = true;
  state.logPage = 1;
  const pending = renderLogResults();
  assert.match(pending, /event-50/);
  assert.match(pending, /Loading logs/);
  assert.match(pending, /data-log-page="0" disabled/);
  assert.match(pending, /data-log-page="2" disabled/);
  state.isLoading = false;
  const loaded = renderLogResults();
  assert.match(loaded, /51–100 of 123/);
  assert.doesNotMatch(loaded, /data-log-page="[02]" disabled/);
});

test('log severity and optional source lines retain their meaning and escape filenames', () => {
  state.logsResponse.entries = [
    { ...state.logsResponse.entries[0], level: 'ERROR', source_file_path: '<error>.rs', line_number: 0 },
    { ...state.logsResponse.entries[1], level: 'WARN', source_file_path: 'warning.rs', line_number: undefined },
    { ...state.logsResponse.entries[2], level: 'INFO', source_file_path: 'info.rs', line_number: 42 },
  ];
  const html = renderLogResults();
  assert.match(html, /class="tag danger-tag">ERROR/);
  assert.match(html, /class="tag warning">WARN/);
  assert.match(html, /class="tag ">INFO/);
  assert.match(html, /&lt;error&gt;\.rs:0<\/p>/);
  assert.match(html, /warning\.rs<\/p>/);
  assert.match(html, /info\.rs:42<\/p>/);
  assert.ok(!html.includes(':undefined'));
  state.logsLoading = true;
  state.logsResponse.entries = [];
  assert.match(renderLogResults(), /Loading recent server events…/);
});

test('user management has a dedicated route and navigation current-page indicator', () => {
  globalThis.location.pathname = '/settings/users';
  state.route = parseRoute();
  assert.deepEqual(state.route, { page: 'settings', section: 'users' });
  const usersHtml = renderSettingsPage();
  assert.match(usersHtml, /href="\/settings\/users" aria-current="page"/);
  assert.match(usersHtml, /data-update-user-id="1"/);
  assert.match(usersHtml, /<details class="settings-add-user">/);
  assert.match(usersHtml, /id="create-user-form"/);
  globalThis.location.pathname = '/settings';
  state.route = parseRoute();
  const html = renderSettingsPage();
  assert.ok(!html.includes('id="create-user-form"'));
  assert.match(html, /<h3>Connection<\/h3>/);
  assert.match(html, /<h3>Storage<\/h3>/);
});

test('provider request policies are absent while saved credentials stay hidden', () => {
  globalThis.location.pathname = '/settings/providers';
  state.route = parseRoute();
  state.settingsResponse.settings.metadata.providers[0].api_key = 'saved-secret';
  const html = renderSettingsPage();
  assert.ok(!html.includes('Request limits and retries'));
  assert.ok(!html.includes('saved-secret'));
  for (const field of ['rate_limit_per_second', 'retry_attempts', 'retry_backoff_ms']) {
    assert.ok(!html.includes(field));
    const form = new FormData();
    form.set(`tmdb_${field}`, '999');
    assert.ok(!(field in buildSettingsFromForm(form).metadata.providers[0]));
  }
  const form = new FormData();
  form.set('tmdb_api_key', 'replacement-key');
  assert.equal(buildSettingsFromForm(form).metadata.providers[0].api_key, 'replacement-key');
  form.set('tmdb_clear_api_key', 'on');
  assert.equal(buildSettingsFromForm(form).metadata.providers[0].api_key, null);
  assert.equal(buildSettingsFromForm(form).metadata.providers[0].clear_api_key, true);
});

test('scheduled task intervals specify days', () => {
  state.route = { page: 'settings', section: 'scheduled' };
  const html = renderSettingsPage();
  assert.equal([...html.matchAll(/<label>Run interval \(days\)/g)].length, 2);
  assert.ok(!html.includes('<label>Run interval\n'));
});

test('icon-only button content has no empty label taking up space beside the icon', () => {
  const html = renderButtonContent('', 'chevron-left');
  assert.match(html, /data-lucide="chevron-left"/);
  assert.doesNotMatch(html, /<span>\s*<\/span>/);
  assert.match(renderButtonContent('Next', 'chevron-right'), /<span>Next<\/span>/);
});

test('API-key removal is an accessible button attached to the password field', () => {
  state.route = { page: 'settings', section: 'providers' };
  const html = renderSettingsPage();
  assert.match(html, /class="input-group api-key-picker"[^>]*aria-label="TheMovieDB API key"/);
  assert.match(html, /id="tmdb-api-key"[^>]*data-api-key-draft/);
  assert.match(html, /name="tmdb_clear_api_key"[^>]*data-api-key-removal hidden/);
  assert.match(html, /data-clear-api-key[^>]*aria-label="Clear TheMovieDB API key"[^>]*>[^]*?data-lucide="x"/);
  assert.match(html, /data-clear-api-key[^>]*aria-label="Clear TheTVDB API key"[^>]*disabled/);
  assert.ok(!html.includes('Clear saved API key'));
});

test('provider artwork comes from API descriptors and resolves against the API server', () => {
  state.route = { page: 'settings', section: 'providers' };
  state.metadataProviders[0].logo_dark_url = '/assets/providers/server-selected.svg';
  const logoUrl = 'http://localhost/assets/providers/server-selected.svg';
  assert.equal(providerAttributionLogo('tmdb'), logoUrl);
  assert.ok(renderSettingsPage().includes(`src="${escapeHtml(logoUrl)}"`));
  globalThis.localStorage = { getItem: () => 'http://backend.test:9191' };
  try {
    assert.equal(providerAttributionLogo('tmdb'), 'http://backend.test:9191/assets/providers/server-selected.svg');
    state.metadataProviders[0].logo_dark_url = undefined;
    assert.equal(providerAttributionLogo('tmdb'), 'http://backend.test:9191/assets/providers/tmdb.svg');
    state.metadataProviders[0].logo_light_url = undefined;
    assert.equal(providerAttributionLogo('tmdb'), undefined);
  } finally {
    globalThis.localStorage = { getItem: () => null };
  }
});

test('every mock provider has working local logos in the development server', async () => {
  const providers = getMockMetadataProviders();
  assert.equal(providers.length, 7);
  const origin = `http://127.0.0.1:${assetServer.address().port}`;
  for (const provider of providers) {
    for (const url of [provider.logo_light_url, provider.logo_dark_url]) {
      assert.match(url, /^\/assets\/providers\/[^/]+\.(svg|png)$/);
      const response = await fetch(`${origin}${url}`);
      assert.equal(response.status, 200, url);
      assert.equal(response.headers.get('content-type'), url.endsWith('.svg') ? 'image/svg+xml' : 'image/png');
      assert.ok((await response.arrayBuffer()).byteLength > 0);
    }
  }
  assert.equal((await fetch(`${origin}/assets/providers/unknown.svg`)).status, 404);
  assert.equal((await fetch(`${origin}/assets/providers/README.md`)).status, 404);
});

test('mock credential saves clear or replace saved keys without exposing plaintext', () => {
  const original = getMockSettings().settings;
  try {
    const next = structuredClone(original);
    next.metadata.providers[0].clear_api_key = true;
    const cleared = updateMockSettings(next).settings;
    assert.equal(cleared.metadata.providers[0].api_key_configured, false);
    assert.ok(!('api_key' in cleared.metadata.providers[0]));
    assert.ok(!('api_key_secret_ref' in cleared.metadata.providers[0]));
    assert.ok(!('clear_api_key' in cleared.metadata.providers[0]));
    cleared.metadata.providers[0].api_key = 'replacement-key';
    const saved = updateMockSettings(cleared).settings;
    assert.equal(saved.metadata.providers[0].api_key_configured, true);
    assert.ok(!('api_key' in saved.metadata.providers[0]));
  } finally {
    updateMockSettings(original);
  }
});

test('image pickers expose labeled folder and remove buttons without native browse controls', () => {
  state.route = { page: 'settings', section: 'users' };
  const html = renderSettingsPage();
  assert.match(html, /data-browse-profile-image[^>]*aria-label="Browse for a profile image"/);
  assert.match(html, /data-remove-profile-image[^>]*aria-label="Remove profile image"/);
  assert.match(html, /data-lucide="folder-open"/);
  assert.match(html, /name="profile_image_file"[^>]*hidden/);
  assert.ok(!html.includes(' /> Remove image'));
});

test('provider ordering uses vertical arrows and logos include a fallback', () => {
  state.route = { page: 'settings', section: 'libraries' };
  const html = renderSettingsPage();
  assert.match(html, /data-provider-move="up"[^<]*>[^]*?data-lucide="chevron-up"/);
  assert.match(html, /data-provider-move="down"[^<]*>[^]*?data-lucide="chevron-down"/);
  state.route = { page: 'settings', section: 'providers' };
  state.metadataProviders[0].logo_dark_url = 'https://example.test/unavailable.svg';
  assert.match(renderSettingsPage(), /provider-logo-fallback/);
  assert.match(renderSettingsPage(), /data-provider-logo/);
});

test('collapsed library cards retain the controls for every configured library', () => {
  globalThis.location.pathname = '/settings/libraries';
  state.route = parseRoute();
  const html = renderSettingsPage();
  const libraries = state.settingsResponse.settings.media.libraries;
  assert.equal([...html.matchAll(/<details class="settings-library-card library-settings-disclosure"/g)].length, libraries.length);
  for (let index = 0; index < libraries.length; index += 1) {
    assert.ok(html.includes(`name="existing_library_paths_${index}"`));
    assert.ok(html.includes(`name="existing_library_name_${index}"`));
  }
  assert.match(html, /<details class="settings-add-library">/);
});

test('saving general settings preserves library, provider and scheduling configuration', () => {
  const original = structuredClone(state.settingsResponse.settings);
  const form = new FormData();
  form.set('address', '127.0.0.2');
  form.set('port', '9192');
  form.set('data_dir', original.general.data_dir);
  const updated = buildSettingsFromForm(form);
  assert.equal(updated.server.address, '127.0.0.2');
  assert.equal(updated.server.port, 9192);
  assert.deepEqual(updated.media, original.media);
  assert.deepEqual(updated.metadata, original.metadata);
  assert.deepEqual(updated.scheduled_tasks, original.scheduled_tasks);
});
