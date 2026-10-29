import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/podcasts', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { loadPodcastDirectory, bindPodcastDiscovery, cancelPodcastDirectorySearch } = await server.ssrLoadModule('/src/app/podcastDiscovery.ts');
const { PODCAST_COUNTRIES } = await server.ssrLoadModule('/src/app/podcastCountries.ts');

beforeEach(() => {
  cancelPodcastDirectorySearch();
  state.route = { page: 'podcasts' };
  state.bootstrap = { current_user: { id: 1 } };
  state.podcasts = { owner: 1, items: [], directory: [], country: 'us', query: '', loading: false, tab: 'discover' };
});

const response = title => Response.json([{ id: title, title, feed_url: 'https://example.com/feed' }]);
const flush = () => new Promise(resolve => setImmediate(resolve));

test('country choices retain every region, use names, and remain alphabetically ordered', () => {
  assert.equal(PODCAST_COUNTRIES.length, 249);
  assert.equal(new Set(PODCAST_COUNTRIES.map(country => country.code)).size, 249);
  const names = PODCAST_COUNTRIES.map(country => country.name);
  assert.deepEqual(names, [...names].sort((a, b) => a.localeCompare(b, 'en')));
  assert.equal(PODCAST_COUNTRIES.find(country => country.code === 'ca').name, 'Canada');
});

test('older directory replies cannot replace the latest query or finish its loading indicator', async () => {
  const originalFetch = globalThis.fetch;
  const pending = [];
  globalThis.fetch = () => new Promise(resolve => pending.push(resolve));
  try {
    state.podcasts.query = 'old';
    const old = loadPodcastDirectory(state, () => true, () => {});
    state.podcasts.query = 'new';
    const latest = loadPodcastDirectory(state, () => true, () => {});
    pending[0](response('Old result')); await old;
    assert.equal(state.podcasts.loading, true);
    assert.deepEqual(state.podcasts.directory, []);
    pending[1](response('New result')); await latest;
    assert.equal(state.podcasts.directory[0].title, 'New result');
    assert.equal(state.podcasts.loading, false);
  } finally { globalThis.fetch = originalFetch; }
});

test('abandoned routes and switched accounts discard directory responses', async () => {
  const originalFetch = globalThis.fetch;
  let complete;
  globalThis.fetch = () => new Promise(resolve => { complete = resolve; });
  try {
    let current = true;
    const route = loadPodcastDirectory(state, () => current, () => {});
    current = false; complete(response('Abandoned route')); await route;
    assert.deepEqual(state.podcasts.directory, []);
    const account = loadPodcastDirectory(state, () => true, () => {});
    state.bootstrap.current_user.id = 2;
    complete(response('Previous account')); await account;
    assert.deepEqual(state.podcasts.directory, []);
  } finally { globalThis.fetch = originalFetch; }
});

test('live search debounces typing, changes country immediately, and clears to popular shows', async () => {
  const originals = { fetch: globalThis.fetch, document: globalThis.document, setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout };
  const form = new EventTarget(), input = new EventTarget(), country = new EventTarget(), clear = new EventTarget();
  input.value = ''; country.value = 'us';
  let focused = false, nextTimer = 0;
  input.focus = () => { focused = true; };
  const timers = new Map(), requests = [];
  const controller = new AbortController();
  globalThis.document = { querySelector: selector => ({ '#podcast-search-form': form, '#podcast-search': input, '#podcast-country': country, '[data-podcast-search-clear]': clear })[selector] };
  globalThis.setTimeout = (callback, delay) => { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; };
  globalThis.clearTimeout = id => timers.delete(id);
  globalThis.fetch = async url => { requests.push(new URL(url, location.origin)); return response('Found show'); };
  try {
    bindPodcastDiscovery(controller.signal, { render() {} });
    input.value = 'sc'; input.dispatchEvent(new Event('input'));
    input.value = 'science'; input.dispatchEvent(new Event('input'));
    assert.equal(requests.length, 0);
    assert.equal(timers.size, 1);
    const [id, timer] = [...timers][0]; timers.delete(id); timer.callback(); await flush();
    assert.equal(requests.length, 1);
    assert.equal(requests[0].searchParams.get('query'), 'science');
    country.value = 'ca'; country.dispatchEvent(new Event('change')); await flush();
    assert.equal(requests[1].searchParams.get('country'), 'ca');
    clear.dispatchEvent(new Event('click')); await flush();
    assert.equal(requests[2].searchParams.get('query'), '');
    assert.equal(input.value, ''); assert.equal(focused, true);
    input.value = 'abandoned'; input.dispatchEvent(new Event('input'));
    state.route = { page: 'home' };
    const queued = [...timers].find(([, value]) => value.delay === 500);
    timers.delete(queued[0]); queued[1].callback(); await flush();
    assert.equal(requests.length, 3);
    state.route = { page: 'podcasts' };
    bindPodcastDiscovery(controller.signal, { render() {} });
    input.value = 'previous account'; input.dispatchEvent(new Event('input'));
    state.bootstrap.current_user.id = 2;
    state.podcasts = { owner: 2, items: [], directory: [], country: 'gb', query: '', loading: false, tab: 'discover' };
    await loadPodcastDirectory(state, () => true, () => {});
    assert.equal(timers.size, 0);
    assert.equal(state.podcasts.loading, false);
    assert.equal(state.podcasts.directory[0].title, 'Found show');
  } finally {
    controller.abort(); cancelPodcastDirectorySearch();
    Object.assign(globalThis, originals);
  }
});
