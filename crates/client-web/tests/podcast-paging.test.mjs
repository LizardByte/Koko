/** Verifies incremental season requests, stale account responses, and retryable page errors. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { JSDOM } from 'jsdom';
globalThis.location = { pathname: '/', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { bindPodcastPaging } = await server.ssrLoadModule('/src/app/podcastPaging.ts');
const flush = () => new Promise(resolve => setImmediate(resolve));
function fixture(t, request) {
  const dom = new JSDOM('<div data-podcast-more="9"><button>Load more</button></div>');
  const original = { document: globalThis.document, IntersectionObserver: globalThis.IntersectionObserver };
  let observe, disconnects = 0, renders = 0;
  globalThis.document = dom.window.document;
  globalThis.IntersectionObserver = class { constructor(callback) { observe = callback; } observe() {} disconnect() { disconnects++; } };
  const item = { id: 9, item_type: 'podcast_season', podcast: { podcast_id: 1 }, child_count: 125, children: Array.from({ length: 50 }, (_, id) => ({ id })) };
  state.bootstrap = { current_user: { id: 1 } }; state.selectedItem = item; state.podcastEpisodeLoading = undefined;
  const controller = new dom.window.AbortController();
  bindPodcastPaging(controller.signal, { render() { renders++; } }, request);
  t.after(() => { controller.abort(); Object.assign(globalThis, original); dom.window.close(); });
  return { item, more: () => observe([{ isIntersecting: true }]), abort: () => controller.abort(), get disconnects() { return disconnects; }, get renders() { return renders; } };
}
test('approaching the last row loads one bounded page and abort disconnects the observer', async t => {
  let finish; const requests = [];
  const f = fixture(t, (id, offset) => { requests.push([id, offset]); return new Promise(resolve => { finish = resolve; }); });
  f.more(); f.more();
  assert.deepEqual(requests, [[9, 50]]);
  finish({ items: Array.from({ length: 50 }, (_, index) => ({ id: index + 50 })), total: 125, has_more: true });
  await flush(); assert.equal(f.item.children.length, 100); assert.equal(f.renders, 1);
  f.abort(); assert.equal(f.disconnects, 1);
});
test('a page from the previous account cannot change the current item', async t => {
  let finish;
  const f = fixture(t, () => new Promise(resolve => { finish = resolve; }));
  f.more(); state.bootstrap = { current_user: { id: 2 } };
  finish({ items: [{ id: 100 }], total: 125 }); await flush();
  assert.equal(f.item.children.length, 50); assert.equal(f.renders, 0);
});
test('failed pages stop automatic retries and the load button retries explicitly', async t => {
  let calls = 0;
  const f = fixture(t, async () => { calls++; if (calls === 1) { throw new Error('Temporary failure'); } return { items: [{ id: 50 }], total: 125 }; });
  f.more(); await flush(); f.more(); await flush(); assert.equal(calls, 1);
  document.querySelector('button').click(); await flush();
  assert.equal(calls, 2); assert.equal(f.item.children.length, 51);
});
