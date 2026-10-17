import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null, removeItem: () => {} };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderHomePage } = await server.ssrLoadModule('/src/app/homeView.ts');
const { renderSettingsPage } = await server.ssrLoadModule('/src/app/settingsView.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { loadRouteData } = await server.ssrLoadModule('/src/app/routeData.ts');
const { ApiError, getLibraries, getApiMode } = await server.ssrLoadModule('/src/api.ts');
const { captureBrowseSession, clearBrowseSessions, restoreBrowseSession } = await server.ssrLoadModule('/src/app/browseSession.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

beforeEach(() => {
  Object.assign(state, {
    route: { page: 'home', libraryId: 1 },
    bootstrap: { has_users: true, current_user: { id: 2, username: 'viewer', admin: false } },
    libraries: mock.getMockLibraries(), home: mock.getMockHome(1), libraryItems: mock.getMockItems(1),
    settingsResponse: mock.getMockSettings(), users: [{ id: 1, username: 'admin', admin: true }],
    systemActivities: [], dashboardItems: [], searchResults: [], searchQuery: '',
    selectedItem: undefined, selectedItemMetadata: undefined, selectedPlayback: undefined,
    isLoading: false, libraryItemsLoading: false, itemManagementOpen: true,
    homeTab: 'recommended', error: undefined,
  });
});

test('ordinary users have no scan, refresh, settings, or metadata management controls', () => {
  const home = renderHomePage();
  assert.doesNotMatch(home, /id="(?:scan-active-library|refresh-active-library-metadata)"/);
  for (const section of ['general', 'libraries', 'providers', 'scheduled', 'users', 'dashboard', 'logs']) {
    state.route = { page: 'settings', section };
    const html = renderSettingsPage();
    assert.match(html, /Administrator access is required/);
    assert.doesNotMatch(html, /settings-form|settings-nav-link|data-settings-create|user-edit-form/);
  }
  const summary = mock.getMockItems(1).find((item) => item.item_type === 'movie');
  state.selectedItem = mock.getMockItem(summary.id);
  state.route = { page: 'item', itemId: summary.id };
  const item = renderItemPage();
  assert.doesNotMatch(item, /id="(?:metadata-search-form|refresh-item-metadata|unlink-item-metadata)"|data-link-metadata/);
  assert.match(item, /Media details/);
});

test('administrators retain library and server management controls', () => {
  state.bootstrap.current_user.admin = true;
  const home = renderHomePage();
  assert.match(home, /id="scan-active-library"/);
  assert.match(home, /id="refresh-active-library-metadata"/);
  state.route = { page: 'settings', section: 'libraries' };
  assert.match(renderSettingsPage(), /data-settings-create="libraries"/);
});

function sourceWithCalls(calls) {
  return new Proxy({}, { get: (_, name) => (...args) => {
    calls.push(name);
    const values = {
      getCapabilities: {}, getLibraries: mock.getMockLibraries(), getMetadataProviders: [],
      getSystemActivities: { activities: [] }, getHome: mock.getMockHome(1), getItems: mock.getMockItems(1),
      getSettings: mock.getMockSettings(), getUsers: [], getLogs: { entries: [] },
      getItem: mock.getMockItem(mock.getMockItems(1)[0].id), getItemMetadata: { matches: [] }, getPlaybackDecision: {},
    };
    return Promise.resolve(values[name]);
  } });
}

test('ordinary browsing and item loading never request administrator data', async () => {
  for (const route of [{ page: 'home' }, { page: 'item', itemId: 1 }]) {
    const calls = [];
    await loadRouteData(state, route, () => {}, () => true, sourceWithCalls(calls));
    assert.ok(calls.includes('getLibraries'));
    assert.ok(calls.includes('getItems'));
    assert.ok(!calls.some((name) => ['getSettings', 'getSystemActivities', 'getUsers', 'getLogs'].includes(name)));
    assert.equal(state.settingsResponse, undefined);
    assert.deepEqual(state.users, []);
  }
});

test('direct settings URLs do not fetch data for ordinary users', async () => {
  for (const section of ['general', 'libraries', 'providers', 'scheduled', 'users', 'dashboard', 'logs']) {
    const calls = [];
    await loadRouteData(state, { page: 'settings', section }, () => {}, () => true, sourceWithCalls(calls));
    assert.deepEqual(calls, []);
  }
});

test('an access denial discards cached media instead of displaying it with an error', async () => {
  const source = sourceWithCalls([]);
  const denied = new Proxy(source, { get: (target, name) => name === 'getHome'
    ? () => Promise.reject(new ApiError(404, 'Not found')) : target[name] });
  await loadRouteData(state, { page: 'home', libraryId: 1 }, () => {}, () => true, denied);
  assert.equal(state.home, undefined);
  assert.deepEqual(state.searchResults, []);
  assert.equal(state.selectedItem, undefined);
});

test('signing out discards saved browsing content for the next login', () => {
  globalThis.document = { activeElement: undefined, querySelector: () => null, querySelectorAll: () => [] };
  globalThis.scrollY = 0;
  captureBrowseSession();
  state.home = undefined;
  assert.equal(restoreBrowseSession(), true);
  clearBrowseSessions();
  state.home = undefined;
  assert.equal(restoreBrowseSession(), false);
  assert.equal(state.home, undefined);
});

test('API access denials stay errors in development without switching to mock libraries', async (t) => {
  let status = 401;
  t.mock.method(globalThis, 'fetch', async () => new Response('Access denied', { status }));
  for (status of [401, 403, 404]) {
    await assert.rejects(getLibraries(), (error) => error instanceof ApiError && error.status === status);
    assert.equal(getApiMode(), 'live');
  }
});

test('expired or deleted accounts lose cached library names and management data', async () => {
  const source = sourceWithCalls([]);
  const denied = new Proxy(source, { get: (target, name) => name === 'getItems'
    ? () => Promise.reject(new ApiError(401, 'Unauthorized')) : target[name] });
  await loadRouteData(state, { page: 'home' }, () => {}, () => true, denied);
  assert.equal(state.bootstrap.current_user, undefined);
  assert.deepEqual(state.libraries, []);
  assert.equal(state.settingsResponse, undefined);
  assert.deepEqual(state.users, []);
});
