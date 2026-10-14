import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { setImmediate } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/', origin: 'http://localhost' };
const storage = new Map();
globalThis.localStorage = { getItem: (key) => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const preferences = await server.ssrLoadModule('/src/app/browsePreferences.ts');
const { renderLibraryTab, renderItemCard } = await server.ssrLoadModule('/src/app/homeView.ts');
const { renderItemPage, renderEpisodeBrowser, renderPersonCreditGroup } = await server.ssrLoadModule('/src/app/itemPersonView.ts');
const { bindHoverPreview } = await server.ssrLoadModule('/src/app/hoverPreview.ts');
const { loadRouteData } = await server.ssrLoadModule('/src/app/routeData.ts');
const { renderLoadingCards } = await server.ssrLoadModule('/src/app/loadingView.ts');
const session = await server.ssrLoadModule('/src/app/browseSession.ts');
const mock = await server.ssrLoadModule('/src/mockApi.ts');

const item = (id, extra = {}) => ({ id, library_id: 1, item_type: 'movie', display_title: `Title ${id}`, genres: [], relative_path: `${id}.mkv`, media_kind: 'video', playable: true, child_count: 0, ...extra });

beforeEach(() => {
  storage.clear();
  Object.assign(state, {
    route: { page: 'home', libraryId: 1 }, bootstrap: { current_user: { id: 1, admin: true } },
    libraries: mock.getMockLibraries(), libraryItems: [], libraryItemsLoading: false,
    home: undefined, selectedItem: undefined, selectedItemMetadata: undefined, selectedPlayback: undefined,
    browsePreferences: { ...preferences.DEFAULT_BROWSE_PREFERENCES }, browseFiltersOpen: false, libraryPage: 0,
    homeTab: 'library', isLoading: false, searchQuery: '', searchResults: [], searchLoading: false, showFullSearchResults: false,
    itemManagementOpen: false, isTrailerMenuOpen: false,
    logsResponse: undefined, logsError: undefined, logsLoading: false, error: undefined,
  });
});

test('sort, genre, year, and watch status compose without mutating source items', () => {
  const items = [item(2, { display_title: 'Beta', release_year: 2001, genres: ['Drama'], playback_completed: true }), item(3, { release_year: 1999, genres: ['Drama'], playback_position_ms: 60_000 }), item(1, { release_year: 1999, genres: ['Drama'] }), item(4, { genres: ['Comedy'] })];
  const defaults = preferences.DEFAULT_BROWSE_PREFERENCES;
  assert.deepEqual(preferences.browseItems(items, { ...defaults, genre: 'Drama', year: '1999', watched: 'unwatched' }).map((entry) => entry.id), [1]);
  assert.deepEqual(preferences.browseItems(items, { ...defaults, watched: 'in-progress' }).map((entry) => entry.id), [3]);
  assert.deepEqual(preferences.browseItems(items, { ...defaults, sort: 'year' }).map((entry) => entry.id), [2, 1, 3, 4]);
  assert.deepEqual(items.map((entry) => entry.id), [2, 3, 1, 4]);
  assert.equal(preferences.browseWatchState(item(5, { watch_count: 1, playback_position_ms: 120_000 })), 'in-progress');
});

test('recent sorting uses modification time and stable title ordering for ties', () => {
  const items = [item(10, { modified_at: 50 }), item(2, { modified_at: 50 }), item(1, { modified_at: 80 }), item(3)];
  assert.deepEqual(preferences.browseItems(items, { ...preferences.DEFAULT_BROWSE_PREFERENCES, sort: 'recent' }).map((entry) => entry.id), [1, 2, 10, 3]);
});

test('preferences are isolated by user and library and damaged storage falls back safely', () => {
  const saved = { ...preferences.DEFAULT_BROWSE_PREFERENCES, genre: 'Drama', density: 'compact', sort: 'recent' };
  preferences.saveBrowsePreferences(saved, 1, 2);
  assert.deepEqual(preferences.loadBrowsePreferences(1, 2), saved);
  assert.deepEqual(preferences.loadBrowsePreferences(1, 3), preferences.DEFAULT_BROWSE_PREFERENCES);
  assert.deepEqual(preferences.loadBrowsePreferences(2, 2), preferences.DEFAULT_BROWSE_PREFERENCES);
  storage.set('koko:browse:1:2', 'broken json');
  assert.deepEqual(preferences.loadBrowsePreferences(1, 2), preferences.DEFAULT_BROWSE_PREFERENCES);
  assert.deepEqual(preferences.normalizeBrowsePreferences({ sort: 'bad', density: 'tiny', year: 'NaN', watched: 'bad' }), preferences.DEFAULT_BROWSE_PREFERENCES);
});

test('library pages render at most 60 cards and the final page contains the remaining titles', () => {
  state.libraryItems = Array.from({ length: 125 }, (_, index) => item(index));
  const first = renderLibraryTab();
  assert.equal([...first.matchAll(/class="media-card /g)].length, 60);
  assert.match(first, /Page 1 of 3/);
  state.libraryPage = 2;
  const last = renderLibraryTab();
  assert.equal([...last.matchAll(/class="media-card /g)].length, 5);
  assert.match(last, /121–125 of 125/);
});

test('empty filtered results keep filter controls available for recovery', () => {
  state.libraryItems = [item(1, { genres: ['Drama'], release_year: 1999 })];
  state.browsePreferences.genre = 'Comedy';
  const html = renderLibraryTab();
  assert.match(html, /No titles match these filters/);
  assert.match(html, /id="reset-library-filters"/);
  assert.match(html, /value="Comedy" selected/);
  assert.match(html, /value="1999"/);
});

test('filters start collapsed and the toggle reports applied filters without counting presentation choices', () => {
  state.libraryItems = [item(1, { genres: ['Drama'], release_year: 1999 })];
  let html = renderLibraryTab();
  assert.match(html, /id="toggle-library-filters" aria-expanded="false"[^>]*aria-label="Filters: No filters applied"/);
  assert.match(html, /id="library-browse-form"[^>]*hidden/);
  state.browsePreferences = { ...state.browsePreferences, sort: 'year', density: 'compact', genre: 'Drama', year: '1999' };
  assert.equal(preferences.activeBrowseFilterCount(state.browsePreferences), 2);
  html = renderLibraryTab();
  assert.match(html, /Filters · 2 active/);
  assert.match(html, /aria-label="Filters: 2 active filters"/);
  assert.match(html, /1 title/);
  state.browseFiltersOpen = true;
  html = renderLibraryTab();
  assert.match(html, /id="toggle-library-filters" aria-expanded="true"/);
  assert.doesNotMatch(html, /id="library-browse-form"[^>]*hidden/);
  state.browsePreferences.watched = 'unwatched';
  assert.equal(preferences.activeBrowseFilterCount(state.browsePreferences), 3);
});

test('cards show the library only in combined browsing and place watch progress on artwork', () => {
  const entry = item(1, { duration_ms: 600_000, playback_position_ms: 120_000, release_year: 1999 });
  const single = renderItemCard(entry);
  assert.doesNotMatch(single, /Movies ·/);
  assert.match(single, /media-card-progress[^>]*20%/);
  assert.doesNotMatch(single, /media-card-kind"/);
  state.route = { page: 'home' };
  const combined = renderItemCard(entry);
  assert.match(combined, /Movies · 1999/);
  assert.match(combined, /media-card-kind"/);
});

test('show pages render a card for each season instead of an episode list', () => {
  state.route = { page: 'item', itemId: 201 };
  state.selectedItem = { ...mock.getMockItem(201), children: [item(204, { item_type: 'season', season_number: 2, display_title: 'Season 2' }), item(202, { item_type: 'season', season_number: 1, display_title: 'Season 1' })] };
  const html = renderItemPage();
  assert.match(html, /<h3>Seasons<\/h3>/);
  assert.match(html, /class="media-card [^"]*"[^>]*data-item-id="202"/);
  assert.match(html, /class="media-card [^"]*"[^>]*data-item-id="204"/);
  assert.ok(html.indexOf('data-item-id="202"') < html.indexOf('data-item-id="204"'));
  assert.doesNotMatch(html, /episode-list|season-tabs/);
});

test('season pages show sibling season tabs and only their own episodes', () => {
  const season = { ...mock.getMockItem(202), library_id: 1, children: [item(205, { parent_id: 202, item_type: 'episode', episode_number: 2, overview: '<script>bad</script>' })] };
  state.libraryItems = [item(204, { parent_id: season.parent_id, item_type: 'season', season_number: 2, display_title: 'Season 2' }), item(203, { parent_id: 204, item_type: 'episode' }), item(888, { parent_id: 999, item_type: 'season', display_title: 'Unrelated season' })];
  const html = renderEpisodeBrowser(season);
  assert.match(html, /class="season-tab [^"]*active" data-item-id="202" aria-current="page"/);
  assert.match(html, /class="season-tab [^"]*" data-item-id="204"/);
  assert.match(html, /data-item-id="205"/);
  assert.doesNotMatch(html, /data-item-id="203"/);
  assert.doesNotMatch(html, /Unrelated season/);
  assert.match(html, /&lt;script&gt;/);
});

test('empty seasons keep season navigation visible before sibling data arrives', () => {
  state.route = { page: 'item', itemId: 202 };
  state.selectedItem = { ...mock.getMockItem(202), children: [] };
  state.libraryItemsLoading = true;
  const html = renderItemPage();
  assert.match(html, /data-item-id="202" aria-current="page"/);
  assert.match(html, /No episodes are available in this season/);
});

test('item technical facts and metadata are behind an accessible disclosure', () => {
  state.route = { page: 'item', itemId: 101 };
  state.selectedItem = mock.getMockItem(101);
  const html = renderItemPage();
  assert.match(html, /<details[^>]*id="item-management"/);
  assert.ok(html.indexOf('class="item-fact-list"') > html.indexOf('<details'));
  assert.ok(html.indexOf('id="metadata-search-form"') > html.indexOf('<details'));
  state.bootstrap.current_user.admin = false;
  assert.doesNotMatch(renderItemPage(), /id="metadata-search-form"/);
});

test('item identity and playback are outside scrolling content with a progress ring and meaningful action icons', () => {
  state.route = { page: 'item', itemId: 101 };
  state.selectedItem = { ...mock.getMockItem(101), rating: 6.8, playback_position_ms: 90_000, playback_duration_ms: 6_000_000 };
  state.selectedItemMetadata = mock.getMockItemMetadata(101);
  const html = renderItemPage();
  const scrollingStart = html.indexOf('id="item-scroll-content"');
  assert.ok(html.indexOf('class="item-cover-column"') < scrollingStart);
  assert.ok(html.indexOf('class="item-identity"') < scrollingStart);
  assert.ok(html.indexOf('class="item-action-dock"') < scrollingStart);
  assert.ok(html.indexOf('class="media-card-progress"') < html.indexOf('class="item-identity"'));
  assert.ok(html.indexOf('class="item-description-section"') > scrollingStart);
  assert.ok(html.indexOf('item-people-section') > scrollingStart);
  assert.doesNotMatch(html, />\d+% watched</);
  assert.match(html, /aria-label="Rating: 6\.8"[^]*?data-lucide="star"/);
  assert.match(html, /data-play-selected-item-start-ms="0"[^]*?data-lucide="rotate-ccw"/);
  assert.match(html, /id="play-item-trailer"[^]*?data-lucide="clapperboard"/);
});

test('the pinned playback summary uses remaining time and completion state without a percentage badge', () => {
  state.route = { page: 'item', itemId: 101 };
  state.selectedItem = { ...mock.getMockItem(101), duration_ms: 600_000, playback_duration_ms: 600_000, playback_position_ms: 120_000, playback_completed: false, watch_count: 0 };
  let html = renderItemPage();
  assert.match(html, /<dt>Runtime<\/dt><dd>10:00<\/dd>/);
  assert.match(html, /<dt>Time remaining<\/dt><dd>8:00<\/dd>/);
  assert.match(html, /<dt>Status<\/dt><dd>In progress<\/dd>/);
  assert.ok(html.indexOf('class="item-playback-summary"') < html.indexOf('class="item-identity"'));
  state.selectedItem.playback_completed = true;
  html = renderItemPage();
  assert.match(html, /<dt>Status<\/dt><dd>Watched<\/dd>/);
  assert.doesNotMatch(html, /<dt>Time remaining<\/dt>/);
  state.selectedItem = mock.getMockItem(201);
  state.libraryItems = [mock.getMockItem(203)];
  html = renderItemPage();
  assert.match(html, /<dt>Next up<\/dt><dd>Mock Episode<\/dd>/);
  assert.match(html, /<dt>Episode runtime<\/dt>/);
  assert.doesNotMatch(html, /<dt>Runtime<\/dt>|<dt>Status<\/dt>/);
});

test('person credits provide explicit buttons for seasons and episodes', () => {
  const html = renderPersonCreditGroup({ root: item(201), seasons: [{ season: item(202), episodes: [item(203)] }] });
  assert.match(html, /data-expand-credit="201"[^>]*aria-expanded="false"[^>]*aria-controls="credit-seasons-201"/);
  assert.match(html, /data-expand-season-credit="202"/);
});

test('loading placeholders are labelled and introduce no focusable controls', () => {
  const html = renderLoadingCards('Loading <titles>');
  assert.match(html, /role="status"/);
  assert.match(html, /Loading &lt;titles&gt;/);
  assert.doesNotMatch(html, /<button|<input|tabindex/);
});

class PreviewElement extends EventTarget {
  isConnected = true;
  hovered = true;
  matches() { return this.hovered; }
}

test('pointer previews wait and cancel when the pointer leaves', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const element = new PreviewElement();
  let calls = 0;
  bindHoverPreview(element, () => { calls += 1; });
  element.dispatchEvent(new Event('mouseenter'));
  t.mock.timers.tick(219);
  assert.equal(calls, 0);
  element.dispatchEvent(new Event('mouseleave'));
  t.mock.timers.tick(500);
  assert.equal(calls, 0);
  element.dispatchEvent(new Event('mouseenter'));
  t.mock.timers.tick(220);
  assert.equal(calls, 1);
});

test('focus previews immediately and consumes a pending mouse preview', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const element = new PreviewElement();
  let calls = 0;
  bindHoverPreview(element, () => { calls += 1; });
  element.dispatchEvent(new Event('mouseenter'));
  element.dispatchEvent(new Event('focus'));
  assert.equal(calls, 1);
  t.mock.timers.tick(1000);
  assert.equal(calls, 1);
});

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function source(overrides = {}) {
  return {
    getCapabilities: async () => ({}), getLibraries: async () => [], getMetadataProviders: async () => [],
    getSettings: async () => mock.getMockSettings(), getSystemActivities: async () => ({ activities: [] }), getUsers: async () => [],
    getHome: async () => ({ shelves: [], collections: [] }), getItems: async () => [],
    getItem: async () => mock.getMockItem(101), getItemMetadata: async () => ({ item_id: 101, matches: [] }),
    getPlaybackDecision: async () => ({}), getPerson: async () => mock.getMockPerson(1), getLogs: async () => ({ log_path: 'test.log', entries: [] }), searchItems: async () => [],
    ...overrides,
  };
}

test('library content arrives independently of a slow hero or capability request', async () => {
  const home = deferred();
  const capabilities = deferred();
  let settingsCalls = 0;
  const pending = loadRouteData(state, state.route, () => {}, () => true, source({ getHome: () => home.promise, getCapabilities: () => capabilities.promise, getItems: async () => [item(7)], getSettings: async () => { settingsCalls += 1; } }));
  await setImmediate();
  assert.equal(state.libraryItems[0].id, 7);
  assert.equal(state.libraryItemsLoading, false);
  assert.equal(state.home, undefined);
  assert.equal(settingsCalls, 0);
  home.resolve({ shelves: [] });
  capabilities.resolve({});
  await pending;
});

test('late replies from an abandoned route cannot overwrite the new page', async () => {
  const titles = deferred();
  let current = true;
  const pending = loadRouteData(state, state.route, () => {}, () => current, source({ getItems: () => titles.promise }));
  await setImmediate();
  current = false;
  state.libraryItems = [item(99)];
  titles.resolve([item(7)]);
  await pending;
  assert.equal(state.libraryItems[0].id, 99);
});

test('a failed library request still publishes available hero content', async () => {
  await loadRouteData(state, state.route, () => {}, () => true, source({ getItems: async () => { throw new Error('offline'); }, getHome: async () => ({ shelves: [{ id: 'ready' }] }) }));
  assert.equal(state.home.shelves[0].id, 'ready');
  assert.match(state.error, /library titles.*offline/);
  assert.equal(state.libraryItemsLoading, false);
});

test('item overview is published before collection and episode data finish', async () => {
  const titles = deferred();
  const pending = loadRouteData(state, { page: 'item', itemId: 101 }, () => {}, () => true, source({ getItems: () => titles.promise }));
  await setImmediate();
  assert.equal(state.selectedItem.id, 101);
  assert.equal(state.libraryItemsLoading, true);
  assert.equal(state.selectedItemMetadata.item_id, 101);
  titles.resolve([]);
  await pending;
  assert.equal(state.libraryItemsLoading, false);
});

test('logs become available while an unrelated settings request is still pending', async () => {
  const settings = deferred();
  state.logsLoading = true;
  const pending = loadRouteData(state, { page: 'settings', section: 'logs' }, () => {}, () => true, source({ getSettings: () => settings.promise }));
  await setImmediate();
  assert.equal(state.logsResponse.log_path, 'test.log');
  assert.equal(state.logsLoading, false);
  settings.resolve(mock.getMockSettings());
  await pending;
});

test('returning to browsing restores the tab, page, scroll, and card focus', () => {
  const main = { scrollTop: 620 };
  const card = { dataset: { itemId: '123' }, matches: (selector) => selector === '.media-card', focus: () => { card.focused = true; } };
  const row = { dataset: { shelfRow: 'recent' }, scrollLeft: 300 };
  const frames = [];
  globalThis.CSS = { escape: (value) => value };
  globalThis.scrollY = 0;
  globalThis.scrollTo = () => {};
  globalThis.requestAnimationFrame = (callback) => frames.push(callback);
  globalThis.document = { activeElement: card, body: { dataset: { inputMode: 'keyboard' } }, querySelector: (selector) => selector === '.main-shell' ? main : selector.includes('data-shelf-row') ? row : card, querySelectorAll: () => [row] };
  state.homeTab = 'library';
  state.libraryPage = 2;
  session.captureBrowseSession();
  state.homeTab = 'recommended';
  state.libraryPage = 0;
  main.scrollTop = 0;
  row.scrollLeft = 0;
  assert.equal(session.restoreBrowseSession(), true);
  session.restoreBrowsePosition();
  frames.forEach((callback) => callback());
  assert.equal(state.homeTab, 'library');
  assert.equal(state.libraryPage, 2);
  assert.equal(main.scrollTop, 620);
  assert.equal(row.scrollLeft, 300);
  assert.equal(card.focused, true);
  assert.equal(session.browsePath({ page: 'browse-detail', libraryId: 1, kind: 'category', key: 'Sci Fi' }), '/libraries/1/items/categories/Sci%20Fi');
});
