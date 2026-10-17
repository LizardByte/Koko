import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/items/101', search: '', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
globalThis.document = { querySelector: (selector) => selector === '#app' ? {} : null };
const server = await createServer({
  root: fileURLToPath(new URL('..', import.meta.url)),
  server: { middlewareMode: true, hmr: false, ws: false },
  plugins: [{ name: 'expose-metadata-polling', enforce: 'pre', transform(code, id) {
    if (id.replaceAll('\\', '/').endsWith('/src/app.ts')) {
      return code + '\nexport { refreshSelectedItemMetadata, refreshPendingMetadataData };';
    }
  } }],
});
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { refreshSelectedItemMetadata, refreshPendingMetadataData } = await server.ssrLoadModule('/src/app.ts');
const { beginMetadataRequest, invalidateMetadataRequests, mutateMetadata } = await server.ssrLoadModule('/src/app/metadataRequests.ts');
const deferred = () => {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
};
const json = (value) => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });
const poll = () => refreshSelectedItemMetadata(101, undefined, beginMetadataRequest());

beforeEach(() => {
  invalidateMetadataRequests();
  Object.assign(state, {
    route: { page: 'item', itemId: 101 },
    bootstrap: { current_user: { admin: true } },
    selectedItem: undefined, selectedItemMetadata: undefined, error: undefined,
  });
});

for (const stage of ['item', 'home']) {
  for (const action of ['unlink', 'rematch', 'newer refresh']) {
    test(`${action} discards an older poll delayed while loading ${stage}`, async () => {
      const started = deferred();
      const delayed = deferred();
      let old = true;
      globalThis.fetch = async (url) => {
        const path = new URL(url, location.origin).pathname;
        const value = path.endsWith('/metadata') ? { matches: old ? [{ title: 'Old match' }] : [] }
          : path.endsWith('/activities') ? { activities: [] }
          : path.endsWith('/101') ? { id: 101, library_id: 1, display_title: old ? 'Old match' : 'Current title' }
          : path.endsWith('/home') ? { shelves: [] } : [];
        if (old && path.endsWith(stage === 'item' ? '/101' : '/home')) {
          started.resolve();
          await delayed.promise;
        }
        return json(value);
      };
      const previous = poll();
      await started.promise;
      if (action !== 'newer refresh') await mutateMetadata(async () => {});
      old = false;
      await poll();
      delayed.resolve();
      assert.equal(await previous, false);
      assert.equal(state.selectedItem.display_title, 'Current title');
      assert.deepEqual(state.selectedItemMetadata.matches, []);
    });
  }
}

test('a poll overlapping a mutation cannot publish even before the mutation finishes', async () => {
  const done = deferred();
  const mutation = mutateMetadata(() => done.promise);
  globalThis.fetch = async (url) => json(String(url).endsWith('/101') ? { id: 101, library_id: 1 } : []);
  assert.equal(await poll(), false);
  assert.equal(state.selectedItem, undefined);
  const overlapping = beginMetadataRequest();
  done.resolve();
  await mutation;
  assert.equal(overlapping(), false);
  assert.equal(beginMetadataRequest()(), true);
});

test('failed mutations still invalidate old work and allow subsequent polls', async () => {
  const previous = beginMetadataRequest();
  await assert.rejects(mutateMetadata(async () => { throw new Error('failure'); }), /failure/);
  assert.equal(previous(), false);
  assert.equal(beginMetadataRequest()(), true);
});

test('an obsolete failed poll cannot replace the current error or schedule another render', async () => {
  const delayed = deferred();
  globalThis.fetch = () => delayed.promise;
  const previous = refreshPendingMetadataData();
  invalidateMetadataRequests();
  state.error = 'Current error';
  delayed.resolve(new Response('Old failure', { status: 503 }));
  await previous;
  assert.equal(state.error, 'Current error');
});

test('route reload invalidation discards a poll even when the same item is visited again', () => {
  const previous = beginMetadataRequest();
  invalidateMetadataRequests();
  assert.equal(previous(), false);
});
