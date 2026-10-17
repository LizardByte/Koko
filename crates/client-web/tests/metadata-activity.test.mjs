import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/libraries/2', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const activities = await server.ssrLoadModule('/src/app/activities.ts');
const { renderLibraryTab } = await server.ssrLoadModule('/src/app/homeView.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
let library;

beforeEach(() => {
  library = { ...mock.getMockLibraries().find((entry) => entry.id === 2), metadata_refresh_total: 100, metadata_refresh_completed: 90, metadata_refresh_pending: 10 };
  state.libraries = [library];
  state.systemActivities = [];
  state.route = { page: 'home', libraryId: 2 };
  state.libraryItems = [{ ...mock.getMockItem(201), metadata_refresh_state: 'pending' }];
  state.libraryItemsLoading = false;
  state.libraryPage = 0;
  state.browseFilter = undefined;
});

function refresh(overrides = {}) {
  return { id: 'tv-refresh', category: 'metadata_refresh', state: 'queued', library_id: 2, item_ids: [201], total_items: 2, completed_items: 0, failed_items: 0, ...overrides };
}

test('persisted pending rows do not create an idle library progress ring', () => {
  assert.equal(activities.libraryRefreshProgress(library), undefined);
  assert.equal(activities.libraryHasActiveMetadataRefresh(2), false);
  assert.doesNotMatch(renderLibraryTab(), /Refreshing metadata \d+\/\d+/);
});

test('queued TV refreshes start from zero and show status beside the library cards', () => {
  state.systemActivities = [refresh()];
  assert.deepEqual(activities.libraryRefreshProgress(library), { total: 2, completed: 0, failed: 0, percent: 0 });
  assert.equal(activities.itemHasActiveMetadataRefresh(state.libraryItems[0]), true);
  const html = renderLibraryTab();
  assert.match(html, /role="status">Refreshing metadata 0\/2/);
  assert.match(html, /loading-spinner/);
});

test('failed items count once toward completion and leave the correct pending count', () => {
  state.systemActivities = [refresh({ state: 'running', total_items: 10, completed_items: 4, failed_items: 2 })];
  assert.equal(activities.activeLibraryPendingRefreshCount(2), 6);
  assert.deepEqual(activities.libraryRefreshProgress(library), { total: 10, completed: 4, failed: 2, percent: 40 });
  assert.match(renderLibraryTab(), /Refreshing metadata 4\/10 · 2 failed/);
});

test('registered automatic show descendants display their own pending loaders', () => {
  state.systemActivities = [refresh({ item_ids: [201, 202, 203], total_items: 3, state: 'running' })];
  for (const id of [202, 203]) {
    const item = { ...mock.getMockItem(id), metadata_refresh_state: 'pending' };
    assert.equal(activities.itemHasActiveMetadataRefresh(item), true);
  }
});

test('finished work and another library do not keep the progress indicator alive', () => {
  state.systemActivities = [refresh({ state: 'completed' }), refresh({ id: 'failed', state: 'failed' }), refresh({ id: 'other', library_id: 3 })];
  assert.equal(activities.libraryRefreshProgress(library), undefined);
  assert.equal(activities.activeLibraryPendingRefreshCount(2), 0);
  assert.doesNotMatch(renderLibraryTab(), /Refreshing metadata \d+\/\d+/);
});
