import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { loadItemSources, renderItemSources } = await server.ssrLoadModule('/src/app/itemSources.ts');
const { renderMetadataSearchResults } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { ApiError } = await server.ssrLoadModule('/src/api.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');
const item = { id: 1, item_type: 'future-group', child_count: 12 };
const page = (offset = 0) => ({ item_id: 1, catalog_title: 'Original <title>', offset, limit: 25,
  total_files: 51, missing_files: 1, total_size: 100, files: [{ id: 10, item_id: 2,
    display_title: 'Local <file>', item_type: 'track', path: 'L:/Music/A&B/01.flac',
    source_root_path: 'L:/Music', relative_path: 'A&B/01.flac', file_size: 100, missing_since: 10, audio_codec: 'flac' }] });

beforeEach(() => Object.assign(state, { route: { page: 'item', itemId: 1 }, selectedItem: item,
  bootstrap: { current_user: { id: 1 } }, itemSources: undefined, metadataSearchResults: [], metadataProviders: [], selectedItemMetadata: undefined }));

test('source inspection loads once on expansion and paginates explicitly', async () => {
  const calls = [];
  let resolve;
  const pending = loadItemSources(() => {}, (undefined), (id, offset) => {
    calls.push([id, offset]); return new Promise((done) => { resolve = done; });
  });
  await loadItemSources(() => {}, undefined, () => { throw new Error('duplicate'); });
  assert.match(renderItemSources(item, 'Library'), /Loading source files/);
  resolve(page()); await pending;
  await loadItemSources(() => {}, undefined, () => { throw new Error('duplicate'); });
  await loadItemSources(() => {}, 25, async (id, offset) => { calls.push([id, offset]); return page(offset); });
  assert.deepEqual(calls, [[1, 0], [1, 25]]);
  const html = renderItemSources(item, 'A&B');
  assert.match(html, /Original &lt;title&gt;/);
  assert.match(html, /L:\/Music\/A&amp;B\/01.flac/);
  assert.match(html, /Library folder/);
  assert.match(html, /Missing since/);
  assert.match(html, /data-item-sources-page="50"/);
});

test('abandoned item or user requests cannot publish paths on another page', async () => {
  let resolve;
  const pending = loadItemSources(() => {}, undefined, () => new Promise((done) => { resolve = done; }));
  state.route = { page: 'item', itemId: 2 };
  state.selectedItem = { id: 2 };
  resolve(page()); await pending;
  assert.equal(state.itemSources.data, undefined);
  assert.doesNotMatch(renderItemSources({ id: 2, item_type: 'movie' }, 'Other'), /Original|A&amp;B/);
  state.route = { page: 'item', itemId: 1 }; state.selectedItem = item;
  state.bootstrap.current_user.id = 2;
  await loadItemSources(() => {}, 0, async () => page());
  assert.equal(state.itemSources.userId, 2);
  state.bootstrap.current_user.id = 3;
  assert.doesNotMatch(renderItemSources(item, 'Other user'), /Original|A&amp;B/);
});

test('access errors clear previously loaded paths and allow explicit retries', async () => {
  await loadItemSources(() => {}, 0, async () => page());
  await loadItemSources(() => {}, 0, async () => { throw new ApiError(404, 'Item unavailable'); });
  assert.equal(state.itemSources.data, undefined);
  assert.match(renderItemSources(item, 'Library'), /Item unavailable/);
  await loadItemSources(() => {}, 0, async () => ({ ...page(), total_files: 0, files: [] }));
  assert.match(renderItemSources(item, 'Library'), /No source files are attached/);
});

test('manual album candidates expose track count agreements and mismatches', () => {
  state.selectedItem = { ...item, item_type: 'album' };
  state.metadataSearchResults = [12, 24].map((track_count) => ({ provider_id: 'musicbrainz',
    external_id: String(track_count), media_type: 'release', title: 'Album', track_count }));
  const html = renderMetadataSearchResults();
  assert.match(html, /12 tracks · matches your 12 tracks/);
  assert.match(html, /24 tracks · your album has 12 tracks/);
});

test('mock source inspection supports groups and bounded pages', () => {
  const sources = mock.getMockItemSources(301, -1, 1000);
  assert.ok(sources.total_files > 0);
  assert.equal(sources.offset, 0);
  assert.equal(sources.limit, 100);
  assert.ok(sources.files.every((file) => file.path.startsWith('C:/Media/Music/')));
});
