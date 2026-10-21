import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null, removeItem: () => {} };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderSimilarItems, renderItemPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { loadRouteData } = await server.ssrLoadModule('/src/app/routeData.ts');
const { ApiError } = await server.ssrLoadModule('/src/api.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

beforeEach(() => {
  Object.assign(state, {
    route: { page: 'item', itemId: 1 }, bootstrap: { current_user: { id: 2, admin: false } },
    selectedItem: undefined, selectedItemMetadata: undefined, selectedPlayback: undefined, similarItems: undefined,
    home: undefined, libraries: mock.getMockLibraries(), libraryItems: [], itemCredits: undefined,
    error: undefined, metadataProviders: [], systemActivities: [], itemSources: undefined,
    isLoading: true, itemMetadataLoading: false, libraryItemsLoading: false,
  });
});

for (const itemType of ['movie', 'show', 'artist', 'album', 'track', 'book', 'book_work', 'audiobook']) {
  test(`the ${itemType} row shows only available distinct ${itemType} items`, () => {
    const template = mock.getMockItem(mock.getMockItems()[0].id);
    state.selectedItem = { ...template, id: 1, item_type: itemType };
    state.similarItems = { itemId: 1, userId: 2, items: [
      { ...template, id: 2, item_type: itemType, display_title: 'Similar item' },
      { ...template, id: 3, item_type: itemType === 'album' ? 'track' : 'album' },
      { ...template, id: 4, item_type: itemType, missing_since: 100 },
      { ...template, id: 1, item_type: itemType },
      { ...template, id: 2, item_type: itemType },
    ] };
    const html = renderSimilarItems();
    assert.match(html, /<h3>More like this<\/h3>/);
    assert.match(html, /data-nav-section="similar"/);
    assert.match(html, /data-shelf-scroll="item-similar:1"/);
    assert.equal((html.match(/data-item-id="2"/g) ?? []).length, 1);
    assert.doesNotMatch(html, /data-item-id="[134]"/);
  });
}

test('empty, stale-item, and stale-account recommendations leave no empty row', () => {
  state.selectedItem = mock.getMockItem(mock.getMockItems()[0].id);
  assert.equal(renderSimilarItems(), '');
  const candidate = { ...state.selectedItem, id: 999 };
  for (const similar of [
    { itemId: state.selectedItem.id, userId: 2, items: [] },
    { itemId: -1, userId: 2, items: [candidate] },
    { itemId: state.selectedItem.id, userId: 3, items: [candidate] },
  ]) {
    state.similarItems = similar;
    assert.equal(renderSimilarItems(), '');
  }
  state.similarItems = { itemId: state.selectedItem.id, userId: 2, items: [candidate] };
  assert.match(renderItemPage(), /<h3>More like this<\/h3>/);
});

test('collections and recommendations precede cast and crew in item section navigation', () => {
  state.selectedItem = mock.getMockItem(101);
  state.selectedItemMetadata = mock.getMockItemMetadata(101);
  const companion = { ...state.selectedItem, id: 102, display_title: 'Collection companion' };
  state.libraryItems = [state.selectedItem, companion];
  state.home = { collections: [
    { id: 'first', name: 'First collection', item_ids: [101, 102] },
    { id: 'second', name: 'Second collection', item_ids: [101, 102] },
  ] };
  state.similarItems = { itemId: 101, userId: 2, items: [companion] };
  const sections = [...renderItemPage().matchAll(/data-nav-section="([^"]+)"/g)].map((match) => match[1]);
  const start = sections.indexOf('collection:first');
  assert.notEqual(start, -1);
  assert.deepEqual(sections.slice(start, start + 5), ['collection:first', 'collection:second', 'similar', 'people', 'crew']);
  state.home.collections = [];
  const withoutCollections = renderItemPage();
  assert.ok(withoutCollections.indexOf('data-nav-section="similar"') < withoutCollections.indexOf('data-nav-section="people"'));
});

function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}

function source(similar, overrides = {}) {
  const item = { ...mock.getMockItem(mock.getMockItems()[0].id), id: 1 };
  const results = { getItem: item, getItemMetadata: { matches: [] }, getHome: { shelves: [], collections: [] }, getItems: [], getLibraries: [], getMetadataProviders: [], getPlaybackDecision: {}, getCapabilities: {} };
  return new Proxy({}, { get: (_, name) => overrides[name] ?? (name === 'getSimilarItems' ? () => similar : async () => results[name]) });
}

for (const endpoint of ['getItem', 'getSimilarItems', 'getHome', 'getItems', 'getItemMetadata', 'getPlaybackDecision']) {
  test(`the complete item page waits for ${endpoint} and publishes its rows together`, async () => {
    const delayed = deferred();
    const item = { ...mock.getMockItem(101), id: 1 };
    const similar = [{ ...item, id: 2 }];
    const results = {
      getItem: item, getSimilarItems: similar,
      getHome: { collections: [{ id: 'collection', name: 'Collection', item_ids: [1, 2] }] },
      getItems: [item, ...similar], getItemMetadata: { ...mock.getMockItemMetadata(101), item_id: 1 }, getPlaybackDecision: {},
    };
    const overrides = Object.fromEntries(Object.entries(results).map(([name, value]) => [name, async () => value]));
    overrides[endpoint] = () => delayed.promise;
    const published = [];
    const pending = loadRouteData(state, state.route, () => {
      if (state.selectedItem) published.push(renderItemPage());
    }, () => true, source(Promise.resolve(similar), overrides));
    await new Promise(setImmediate);
    assert.equal(state.selectedItem, undefined);
    assert.equal(state.selectedItemMetadata, undefined);
    assert.equal(state.home, undefined);
    assert.equal(state.similarItems, undefined);
    assert.equal(published.length, 0);
    delayed.resolve(results[endpoint]);
    await pending;
    assert.equal(state.selectedItem.id, 1);
    assert.equal(state.itemMetadataLoading, false);
    assert.equal(state.libraryItemsLoading, false);
    assert.ok(published.length > 0);
    for (const html of published) {
      assert.match(html, /data-nav-section="collection:collection"/);
      assert.match(html, /data-nav-section="similar"/);
      assert.match(html, /data-nav-section="people"/);
      assert.match(html, /data-nav-section="crew"/);
    }
  });
}

test('a late recommendation reply cannot publish an abandoned item page', async () => {
  const delayed = deferred();
  let current = true;
  const pending = loadRouteData(state, state.route, () => {}, () => current, source(delayed.promise));
  await new Promise(setImmediate);
  assert.equal(state.selectedItem, undefined);
  assert.equal(state.similarItems, undefined);
  current = false;
  state.route = { page: 'item', itemId: 99 };
  delayed.resolve([{ id: 2, item_type: 'movie' }]);
  await pending;
  assert.equal(state.selectedItem, undefined);
  assert.equal(state.similarItems, undefined);
});

test('same-item revalidation retains its recommendations until the complete update arrives', async () => {
  state.selectedItem = { ...mock.getMockItem(101), id: 1 };
  const original = { itemId: 1, userId: 2, items: [{ ...state.selectedItem, id: 2 }] };
  state.similarItems = original;
  const delayed = deferred();
  const pending = loadRouteData(state, state.route, () => {}, () => true, source(delayed.promise));
  await new Promise(setImmediate);
  assert.equal(state.similarItems, original);
  assert.match(renderItemPage(), /data-item-id="2"/);
  delayed.resolve([{ ...state.selectedItem, id: 3 }]);
  await pending;
  assert.equal(state.similarItems.items[0].id, 3);
});

test('a late recommendation reply cannot cross account changes', async () => {
  const delayed = deferred();
  const pending = loadRouteData(state, state.route, () => {}, () => true, source(delayed.promise));
  await new Promise(setImmediate);
  state.bootstrap.current_user.id = 3;
  delayed.resolve([{ id: 2, item_type: 'movie' }]);
  await pending;
  assert.equal(state.selectedItem, undefined);
  assert.equal(state.similarItems, undefined);
});

test('a recommendation request failure leaves usable details without an empty row', async () => {
  await loadRouteData(state, state.route, () => {}, () => true, source(Promise.reject(new Error('offline'))));
  assert.equal(state.selectedItem.id, 1);
  assert.equal(state.error, undefined);
  assert.equal(renderSimilarItems(), '');
});

test('access rejection clears previously loaded recommendation cards and item details', async () => {
  const delayed = deferred();
  const pending = loadRouteData(state, state.route, () => {}, () => true, source(delayed.promise.then(() => { throw new ApiError(403, 'Forbidden'); })));
  await new Promise(setImmediate);
  delayed.resolve();
  await pending;
  assert.equal(state.selectedItem, undefined);
  assert.equal(state.similarItems, undefined);
});
