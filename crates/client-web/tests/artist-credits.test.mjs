import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { setImmediate } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { renderItemPage, renderPersonPage } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { loadRouteData } = await server.ssrLoadModule('/src/app/routeData.ts');
const { ApiError } = await server.ssrLoadModule('/src/api.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

beforeEach(() => Object.assign(state, {
  route: { page: 'item', itemId: 301 }, bootstrap: { current_user: { id: 1, admin: false } },
  selectedItem: mock.getMockItem(301), selectedItemMetadata: undefined, selectedPerson: undefined,
  itemCredits: undefined, similarItems: undefined, home: undefined, libraryItems: [], metadataProviders: [], error: undefined,
  isLoading: false,
}));

function source(overrides = {}) {
  return {
    getCapabilities: async () => ({}), getLibraries: async () => mock.getMockLibraries(), getMetadataProviders: async () => [],
    getHome: async () => mock.getMockHome(3), getItems: async () => [], getItem: async () => mock.getMockItem(301),
    getItemMetadata: async () => ({ item_id: 301, matches: [] }), getPlaybackDecision: async () => ({}),
    getItemCredits: async () => mock.getMockPerson(1).credits, ...overrides,
  };
}

test('artist credits and movie links are ready when the artist page is published', async () => {
  state.selectedItem = undefined;
  let resolve;
  const pendingCredits = new Promise((done) => { resolve = done; });
  const loading = loadRouteData(state, state.route, () => {}, () => true, source({ getItemCredits: () => pendingCredits }));
  await setImmediate();
  assert.equal(state.selectedItem, undefined);
  assert.match(renderItemPage(), /Loading item details/);
  resolve(mock.getMockPerson(1).credits);
  await loading;
  assert.equal(state.selectedItem.id, 301);
  const html = renderItemPage();
  assert.match(html, /<h3>Movies and shows <span class="item-row-count">· [1-9]\d*<\/span><\/h3>/);
  assert.match(html, /Mock Movie/);
  assert.match(html, /data-item-id="101"/);
  assert.doesNotMatch(html, /Loading screen credits/);
});

test('artist credits retain grouped show, season and episode navigation', () => {
  const episode = mock.getMockItem(203);
  const credit = {
    credit: { id: 1, media_item_id: 203, sort_order: 0 }, item: episode, hierarchy: episode.hierarchy,
  };
  state.itemCredits = { itemId: 301, loading: false, credits: [credit, { ...credit, credit: { ...credit.credit, id: 2 } }] };
  const html = renderItemPage();
  assert.match(html, /<h3>Movies and shows <span class="item-row-count">· 1<\/span><\/h3>/);
  assert.match(html, /data-expand-credit="201"/);
  assert.match(html, /data-expand-season-credit="202"/);
  assert.match(html, /data-item-id="203"/);
});

test('artist row counts sit beside their titles and recommendations precede screen credits', () => {
  const artist = state.selectedItem;
  state.selectedItem = { ...artist, children: [...artist.children, { ...artist.children[0], id: 303 }] };
  state.similarItems = { itemId: artist.id, userId: 1, items: [{ ...artist, id: 401 }] };
  state.itemCredits = { itemId: artist.id, loading: false, credits: mock.getMockPerson(1).credits };
  const html = renderItemPage();
  assert.match(html, /<h3>Albums <span class="item-row-count">· 2<\/span><\/h3>/);
  assert.match(html, /<h3>More like this <span class="item-row-count">· 1<\/span><\/h3>/);
  const sections = [...html.matchAll(/data-nav-section="([^"]+)"/g)].map((match) => match[1]);
  assert.deepEqual(sections.filter((section) => ['children', 'similar', 'screen-credits'].includes(section)), ['children', 'similar', 'screen-credits']);
  assert.doesNotMatch(html, /<span class="muted">\d+ (?:items|titles)<\/span>/);
});

test('artists omit empty album and screen-credit rows, including unavailable and stale credits', () => {
  state.selectedItem = { ...state.selectedItem, children: [] };
  for (const credits of [
    undefined,
    { itemId: 301, loading: false, credits: [] },
    { itemId: 301, loading: true, credits: [] },
    { itemId: 301, loading: false, credits: [], error: 'Unavailable' },
    { itemId: 999, loading: false, credits: mock.getMockPerson(1).credits },
  ]) {
    state.itemCredits = credits;
    const html = renderItemPage();
    assert.doesNotMatch(html, /data-nav-section="(?:children|screen-credits)"/);
    assert.doesNotMatch(html, /Movies and shows|No matching movies or shows|Loading screen credits/);
  }
});

test('screen people link to their matching music artists', () => {
  state.selectedPerson = mock.getMockPerson(1);
  const html = renderPersonPage();
  assert.match(html, /<h3>Albums in your libraries <span class="item-row-count">· 1<\/span><\/h3>/);
  assert.match(html, /<h3>Credits <span class="item-row-count">· [1-9]\d*<\/span><\/h3>/);
  assert.match(html, /data-item-id="302"/);
  assert.doesNotMatch(html, /data-item-id="301"/);
  state.selectedPerson.albums = [];
  assert.doesNotMatch(renderPersonPage(), /Albums in your libraries/);
});

test('late artist replies cannot populate another route and ordinary items skip the credits request', async () => {
  let resolve;
  let current = true;
  const reply = new Promise((done) => { resolve = done; });
  const pending = loadRouteData(state, state.route, () => {}, () => current, source({ getItemCredits: () => reply }));
  await setImmediate();
  current = false;
  state.itemCredits = undefined;
  resolve(mock.getMockPerson(1).credits);
  await pending;
  assert.equal(state.itemCredits, undefined);
  await loadRouteData(state, { page: 'item', itemId: 101 }, () => {}, () => true, source({
    getItem: async () => mock.getMockItem(101), getItemCredits: () => assert.fail('Movie credits should not be requested'),
  }));
  assert.equal(state.itemCredits, undefined);
});

test('credit access failures clear previously visible item data', async () => {
  await loadRouteData(state, state.route, () => {}, () => true, source({
    getItemCredits: async () => { throw new ApiError(403, 'Denied'); },
  }));
  assert.equal(state.itemCredits, undefined);
  assert.equal(state.selectedItem, undefined);
});

test('groups and solo artists expose one-person rows using the existing person navigation', () => {
  const person = { id: 50, person_id: 50, name: 'Musician <name>', role: 'Member · bass · 2005–present', department: 'Members', sort_order: 0 };
  state.selectedItemMetadata = { item_id: 301, matches: [{ people: [person] }] };
  let html = renderItemPage();
  assert.match(html, /data-nav-label="Members"/);
  assert.match(html, /data-person-id="50"/);
  assert.match(html, /Musician &lt;name&gt;/);
  assert.match(html, /Member · bass · 2005–present/);
  assert.doesNotMatch(html, /data-nav-label="Crew"/);
  person.department = 'Artists';
  person.role = 'Artist';
  state.selectedItem.overview = '   ';
  html = renderItemPage();
  assert.match(html, /data-nav-label="People"/);
  assert.match(html, /data-person-id="50"/);
  assert.doesNotMatch(html, /item-description-section|No description is stored/);
  state.selectedItemMetadata.matches[0].overview = 'Available artist description';
  assert.match(renderItemPage(), /Available artist description/);
});

test('member person pages keep group memberships separate from individual screen credits', () => {
  state.selectedPerson = { ...mock.getMockPerson(1), groups: [{
    item: mock.getMockItem(301), hierarchy: [], credit: { id: 50, department: 'Members', role: 'Former member · guitar · 1990–2000', sort_order: 0 },
  }] };
  const html = renderPersonPage();
  assert.match(html, /<h3>Groups in your libraries <span class="item-row-count">· 1<\/span><\/h3>/);
  assert.match(html, /Former member · guitar · 1990–2000/);
  assert.match(html, /data-nav-section="credits"/);
  assert.match(html, /data-item-id="101"/);
});
