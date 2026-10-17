import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/items/101', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const mock = await server.ssrLoadModule('/src/mockApi.ts');
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');

function withQueuedWork(run) {
  const timers = [];
  const previous = globalThis.setTimeout;
  globalThis.setTimeout = (callback) => { timers.push(callback); return timers.length; };
  try { run(timers); } finally { globalThis.setTimeout = previous; }
}
const match = { provider_id: 'tmdb', external_id: '603', media_type: 'movie' };

test('rematching immediately clears metadata and waits for queued replacement work', () => withQueuedWork((timers) => {
  const before = { ...mock.getMockItem(101) };
  const pending = mock.linkMockItemMetadata(101, match);
  const cleared = mock.getMockItem(101);
  assert.equal(pending.refresh_state, 'pending');
  assert.equal(pending.title, undefined);
  assert.equal(pending.artwork_url, undefined);
  assert.equal(cleared.has_metadata, false);
  assert.equal(cleared.overview, undefined);
  assert.equal(cleared.poster_url, undefined);
  assert.equal(cleared.theme_song_url, undefined);
  assert.deepEqual(cleared.extras, []);
  assert.deepEqual(cleared.genres, []);
  assert.equal(cleared.relative_path, before.relative_path);
  assert.equal(cleared.duration_ms, before.duration_ms);
  assert.equal(cleared.playback_position_ms, before.playback_position_ms);
  timers.forEach((callback) => callback());
  assert.equal(mock.getMockItem(101).display_title, 'The Matrix');
  assert.equal(mock.getMockItem(101).has_metadata, true);
  assert.equal(mock.getMockItemMetadata(101).matches[0].refresh_state, 'fresh');
}));

test('Unlink clears the item and rejects earlier rematch and refresh completions', () => withQueuedWork((timers) => {
  mock.linkMockItemMetadata(101, match);
  mock.refreshMockItemMetadata(101);
  mock.refreshMockLibraryMetadata(1);
  mock.unlinkMockItemMetadata(101);
  timers.forEach((callback) => callback());
  assert.deepEqual(mock.getMockItemMetadata(101).matches, []);
  assert.equal(mock.getMockItem(101).display_title, 'Mock Movie');
  assert.equal(mock.getMockItem(101).has_metadata, false);
  assert.equal(mock.getMockItem(101).metadata_refresh_state, undefined);
  assert.equal(mock.getMockItem(101).overview, undefined);
}));

test('unlinking a show clears all season and episode metadata, including nested cards', () => {
  mock.unlinkMockItemMetadata(201);
  for (const id of [201, 202, 203]) {
    const item = mock.getMockItem(id);
    assert.equal(item.has_metadata, false);
    assert.deepEqual(item.genres, []);
    assert.equal(item.poster_url, undefined);
    assert.equal(item.theme_song_url, undefined);
    assert.deepEqual(mock.getMockItemMetadata(id).matches, []);
    for (const reference of [...item.children, ...item.hierarchy]) {
      assert.deepEqual(reference.genres, []);
    }
  }
});

test('administrators can unlink a pending match and the action disappears after unlinking', () => withQueuedWork(() => {
  mock.linkMockItemMetadata(101, match);
  Object.assign(state, {
    route: { page: 'item', itemId: 101 }, selectedItem: mock.getMockItem(101),
    selectedItemMetadata: mock.getMockItemMetadata(101), selectedPlayback: undefined,
    bootstrap: { current_user: { id: 1, admin: true } }, libraries: mock.getMockLibraries(),
    libraryItems: mock.getMockItems(1), systemActivities: [], metadataProviders: mock.getMockMetadataProviders(),
    isLoading: false, itemManagementOpen: true,
  });
  assert.match(renderItemPage(), /id="unlink-item-metadata"/);
  assert.match(renderItemPage(), /Unlink metadata/);
  mock.unlinkMockItemMetadata(101);
  state.selectedItem = mock.getMockItem(101);
  state.selectedItemMetadata = mock.getMockItemMetadata(101);
  assert.doesNotMatch(renderItemPage(), /id="unlink-item-metadata"/);
}));
