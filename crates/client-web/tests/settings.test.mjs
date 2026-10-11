import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

// Load the actual TypeScript views through the existing Vite dependency.
globalThis.location = { pathname: '/settings', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({
  root: fileURLToPath(new URL('..', import.meta.url)),
  server: { middlewareMode: true },
});
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderLogResults, renderLogViewer } = await server.ssrLoadModule('/src/app/logView.ts');
const { renderSettingsPage, buildSettingsFromForm } = await server.ssrLoadModule('/src/app/settingsView.ts');
const { parseRoute } = await server.ssrLoadModule('/src/app/routes.ts');
const { getMockSettings, getMockMetadataProviders } = await server.ssrLoadModule('/src/mockApi.ts');

beforeEach(() => {
  globalThis.location.pathname = '/settings';
  state.route = parseRoute();
  state.settingsResponse = getMockSettings();
  state.metadataProviders = getMockMetadataProviders();
  state.bootstrap = { current_user: { id: 1, username: 'admin', admin: true } };
  state.users = [state.bootstrap.current_user];
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

test('provider request settings remain in the form and saved credentials stay hidden', () => {
  globalThis.location.pathname = '/settings/providers';
  state.route = parseRoute();
  state.settingsResponse.settings.metadata.providers[0].api_key = 'saved-secret';
  const html = renderSettingsPage();
  assert.match(html, /<details class="provider-request-settings">/);
  assert.ok(!html.includes('saved-secret'));
  for (const provider of state.settingsResponse.settings.metadata.providers.filter(provider => provider.id !== 'local_nfo')) {
    assert.ok(html.includes(`name="${provider.id}_retry_attempts"`));
  }
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
