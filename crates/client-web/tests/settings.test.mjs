import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { createServer as createHttpServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
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
const { renderSettingsPage, buildSettingsFromForm, metadataProviderCheckboxes } = await server.ssrLoadModule('/src/app/settingsView.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { getMockSettings, getMockMetadataProviders, updateMockSettings } = await server.ssrLoadModule('/src/mockApi.ts');
const { providerAttributionLogo } = await server.ssrLoadModule('/src/app/providers.ts');
const { escapeHtml } = await server.ssrLoadModule('/src/app/format.ts');
const { renderButtonContent } = await server.ssrLoadModule('/src/app/ui.ts');

beforeEach(() => {
  globalThis.location.pathname = '/settings';
  state.route = parseRoute();
  state.settingsResponse = getMockSettings();
  state.settingsCreation = undefined;
  state.metadataProviders = getMockMetadataProviders();
  state.selectedItemMetadata = undefined;
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

test('pages explicitly request the application favicon, which is served as icon bytes', async () => {
  const base = `http://127.0.0.1:${assetServer.address().port}`;
  const html = await (await fetch(`${base}/settings/libraries`)).text();
  assert.match(html, /<link\b[^>]*rel="icon"[^>]*href="\/favicon\.ico"/);
  const response = await fetch(`${base}/favicon.ico`);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'image/x-icon');
  const bytes = Buffer.from(await response.arrayBuffer());
  assert.deepEqual(bytes, await readFile(new URL('../../../assets/icon.ico', import.meta.url)));
  const head = await fetch(`${base}/favicon.ico`, { method: 'HEAD' });
  assert.equal(head.status, 200);
  assert.equal(head.headers.get('content-type'), 'image/x-icon');
  assert.equal((await head.arrayBuffer()).byteLength, 0);
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
  state.route = { page: 'settings', section: 'logs' };
  state.logsLoading = true;
  const html = renderSettingsPage();
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
  state.logsLoading = true;
  state.logPage = 1;
  const pending = renderLogResults();
  assert.match(pending, /event-50/);
  assert.match(pending, /Loading logs/);
  assert.match(pending, /data-log-page="0" disabled/);
  assert.match(pending, /data-log-page="2" disabled/);
  state.isLoading = false;
  state.logsLoading = false;
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
  assert.match(usersHtml, /id="add-user-panel" hidden/);
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

test('all declared mock provider logos work in the development server', async () => {
  const providers = getMockMetadataProviders();
  assert.equal(providers.length, 9);
  const origin = `http://127.0.0.1:${assetServer.address().port}`;
  for (const provider of providers) {
    for (const url of [provider.logo_light_url, provider.logo_dark_url].filter(Boolean)) {
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

test('Fanart.tv is selectable per music library only alongside MusicBrainz', () => {
  const selected = metadataProviderCheckboxes('providers', ['musicbrainz', 'fanart'], 'music');
  const fanartInput = selected.match(/<input[^>]*value="fanart"[^>]*>/)?.[0];
  assert.match(fanartInput, /checked/);
  assert.doesNotMatch(fanartInput, /disabled/);
  assert.match(selected, /data-provider-role="secondary" data-extends-provider-ids="musicbrainz"/);
  const noMusicBrainz = metadataProviderCheckboxes('providers', ['fanart'], 'music');
  assert.match(noMusicBrainz.match(/<input[^>]*value="fanart"[^>]*>/)?.[0], /disabled/);
  assert.doesNotMatch(metadataProviderCheckboxes('providers', [], 'movies'), /value="fanart"/);
});

test('LRCLIB is a keyless library-selectable music agent extending MusicBrainz matches', () => {
  const html = metadataProviderCheckboxes('providers', ['musicbrainz', 'lrclib'], 'music');
  const input = html.match(/<input[^>]*value="lrclib"[^>]*>/)?.[0];
  assert.match(input, /checked/);
  assert.doesNotMatch(input, /disabled/);
  assert.match(html, /data-provider-option="lrclib" data-provider-role="secondary" data-extends-provider-ids="musicbrainz"/);
  assert.match(metadataProviderCheckboxes('providers', ['lrclib'], 'music').match(/<input[^>]*value="lrclib"[^>]*>/)?.[0], /disabled/);
  assert.match(html, /\/assets\/providers\/lrclib.png/);
  assert.doesNotMatch(metadataProviderCheckboxes('providers', [], 'movies'), /value="lrclib"/);
  const index = state.settingsResponse.settings.media.libraries.findIndex((library) => library.kind === 'music');
  const form = new FormData();
  form.set(`existing_library_name_${index}`, 'Music');
  form.set(`existing_library_paths_${index}`, '/music');
  form.append(`existing_library_metadata_provider_${index}`, 'lrclib');
  form.append(`existing_library_metadata_provider_${index}`, 'musicbrainz');
  assert.deepEqual(buildSettingsFromForm(form).media.libraries[index].metadata_providers, ['lrclib', 'musicbrainz']);
  state.route = { page: 'settings', section: 'providers' };
  const settingsHtml = renderSettingsPage();
  assert.match(settingsHtml, /LRCLIB/);
  assert.match(settingsHtml, /Playback reads saved lyrics/);
  assert.doesNotMatch(settingsHtml, /name="lrclib_api_key"/);
});

test('secondary providers retain their saved priority ahead of primary providers', () => {
  const html = metadataProviderCheckboxes('providers', ['fanart', 'musicbrainz'], 'music');
  assert.ok(html.indexOf('data-provider-option="fanart"') < html.indexOf('data-provider-option="musicbrainz"'));
  const fanart = html.split('data-provider-option="fanart"')[1].split('data-provider-option=')[0];
  assert.match(fanart, /Priority 1 · Secondary/);
  assert.match(fanart, /aria-label="Move Fanart.tv up"/);
  assert.match(fanart, /aria-label="Move Fanart.tv down"/);
  state.route = { page: 'settings', section: 'libraries' };
  const libraries = state.settingsResponse.settings.media.libraries;
  const index = libraries.findIndex((library) => library.kind === 'music');
  const form = new FormData();
  form.set(`existing_library_name_${index}`, libraries[index].name);
  form.set(`existing_library_paths_${index}`, libraries[index].path);
  form.append(`existing_library_metadata_provider_${index}`, 'fanart');
  form.append(`existing_library_metadata_provider_${index}`, 'musicbrainz');
  const saved = buildSettingsFromForm(form).media.libraries[index].metadata_providers;
  assert.deepEqual(saved, ['fanart', 'musicbrainz']);
  const reopened = metadataProviderCheckboxes('providers', saved, 'music');
  assert.ok(reopened.indexOf('data-provider-option="fanart"') < reopened.indexOf('data-provider-option="musicbrainz"'));
});

test('Fanart.tv settings accept personal keys without exposing saved credentials', () => {
  state.route = { page: 'settings', section: 'providers' };
  const provider = state.settingsResponse.settings.metadata.providers.find((entry) => entry.id === 'fanart');
  provider.api_key = 'saved-personal-secret';
  let html = renderSettingsPage();
  assert.match(html, /Personal API key/);
  assert.match(html, /https:\/\/fanart.tv\/get-an-api-key\//);
  assert.doesNotMatch(html, /saved-personal-secret/);
  assert.doesNotMatch(html, /name="fanart_project/);
  const form = new FormData();
  form.set('fanart_api_key', 'new-personal-secret');
  assert.equal(buildSettingsFromForm(form).metadata.providers.find((entry) => entry.id === 'fanart').api_key, 'new-personal-secret');
  form.set('fanart_clear_api_key', 'on');
  assert.equal(buildSettingsFromForm(form).metadata.providers.find((entry) => entry.id === 'fanart').clear_api_key, true);
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

test('library metadata sources show descriptor logos for existing and new libraries after viewing another media type', () => {
  state.route = { page: 'settings', section: 'libraries' };
  state.settingsCreation = 'libraries';
  // Item-specific descriptors need not include the providers used by other libraries.
  state.selectedItemMetadata = { item_id: 301, matches: [], providers: state.metadataProviders.filter((provider) => provider.id === 'musicbrainz') };
  const html = renderSettingsPage();
  for (const prefix of ['library_metadata_provider', 'existing_library_metadata_provider_0']) {
    const label = [...html.matchAll(/<label class="checkbox-inline">([^]*?)<\/label>/g)]
      .find((match) => match[1].includes(`name="${prefix}"`) && match[1].includes('value="tmdb"'))?.[1];
    assert.ok(label, `Missing ${prefix} provider checkbox`);
    assert.match(label, /provider-settings-logo provider-source-logo/);
    assert.match(label, /data-provider-logo src="http:\/\/localhost\/assets\/providers\/tmdb.svg" alt=""/);
    assert.match(label, /aria-hidden="true"/);
  }
  const music = metadataProviderCheckboxes('music', ['musicbrainz', 'fanart'], 'music');
  for (const file of ['musicbrainz.svg', 'fanart.png']) assert.ok(music.includes(`/assets/providers/${file}`));
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
  assert.match(html, /id="add-library-panel" hidden/);
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

test('each configuration page has one top Save action associated with its own form', () => {
  for (const section of ['general', 'libraries', 'providers', 'scheduled']) {
    state.route = { page: 'settings', section };
    const html = renderSettingsPage();
    assert.equal([...html.matchAll(/type="submit" form="settings-form"/g)].length, 1);
    assert.ok(html.indexOf('form="settings-form"') < html.indexOf('<form id="settings-form"'));
    assert.match(html, /class="settings-action-dock"/);
    assert.match(html, /data-sticky-identity hidden/);
    assert.doesNotMatch(html, /Save (library settings|provider settings|scheduled tasks)/);
  }
  state.settingsResponse = undefined;
  assert.match(renderSettingsPage(), /type="submit" form="settings-form" disabled/);
});

test('Add actions expose forms above the lists and retain their open state across renders', () => {
  for (const [section, panel, form, list] of [
    ['libraries', 'add-library-panel', 'add-library-form', '<form id="settings-form"'],
    ['users', 'add-user-panel', 'create-user-form', '<div class="user-list"'],
  ]) {
    state.route = { page: 'settings', section };
    const collapsed = renderSettingsPage();
    assert.match(collapsed, new RegExp(`data-settings-create="${section}" aria-controls="${panel}" aria-expanded="false"`));
    assert.match(collapsed, new RegExp(`id="${panel}" hidden`));
    state.settingsCreation = section;
    const expanded = renderSettingsPage();
    assert.match(expanded, /aria-expanded="true"/);
    assert.doesNotMatch(expanded, new RegExp(`id="${panel}" hidden`));
    assert.ok(expanded.indexOf(`id="${form}"`) < expanded.indexOf(list));
    assert.match(expanded, /data-close-settings-create/);
    assert.match(expanded, new RegExp(`type="submit" form="${form}"`));
    assert.match(expanded, /data-settings-default-actions hidden/);
    assert.doesNotMatch(expanded, /data-settings-creation-actions hidden/);
  }
});

test('cache clearing and log refresh share the top action bar, with no duplicate controls', () => {
  state.route = { page: 'settings', section: 'providers' };
  let html = renderSettingsPage();
  assert.equal([...html.matchAll(/id="clear-metadata-cache"/g)].length, 1);
  assert.ok(html.indexOf('id="clear-metadata-cache"') < html.indexOf('<form id="settings-form"'));
  state.route = { page: 'settings', section: 'logs' };
  html = renderSettingsPage();
  assert.equal([...html.matchAll(/id="refresh-log-viewer"/g)].length, 1);
  assert.ok(html.indexOf('id="refresh-log-viewer"') < html.indexOf('id="log-viewer-panel-root"'));
});

test('background activity precedes the metadata dashboard', () => {
  state.route = { page: 'settings', section: 'dashboard' };
  const html = renderSettingsPage();
  assert.ok(html.indexOf('id="system-activities-panel-root"') < html.indexOf('id="metadata-dashboard-panel-root"'));
  assert.ok(html.indexOf('Background activities') < html.indexOf('Metadata dashboard'));
});


test('podcast storage settings persist independently of other settings forms', () => {
  const form = new FormData();
  form.set('podcast_download_dir', 'D:/Podcast library');
  let settings = buildSettingsFromForm(form);
  assert.deepEqual(settings.podcasts, { download_dir: 'D:/Podcast library', download_new_episodes: false });
  state.settingsResponse.settings = settings;
  assert.deepEqual(buildSettingsFromForm(new FormData()).podcasts, settings.podcasts);
  form.set('podcast_download_new', 'on');
  assert.equal(buildSettingsFromForm(form).podcasts.download_new_episodes, true);
  assert.match(renderSettingsPage(), /Podcast download directory/);
});
