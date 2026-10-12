import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { syncPinnedHomeHeader } = await server.ssrLoadModule('/src/app/pinnedHeader.ts');

let header;
const properties = new Map();
const observers = [];
globalThis.document = {
  querySelector: () => header ?? null,
  documentElement: { style: {
    setProperty: (name, value) => properties.set(name, value),
    removeProperty: (name) => properties.delete(name),
  } },
};
globalThis.ResizeObserver = class {
  constructor(callback) { this.callback = callback; observers.push(this); }
  observe(target) { this.target = target; }
  disconnect() { this.disconnected = true; }
};
const makeHeader = (height) => ({ height, getBoundingClientRect() { return { height: this.height }; } });
const offset = () => properties.get('--home-pinned-header-height');

beforeEach(() => {
  header = undefined;
  syncPinnedHomeHeader();
  observers.length = 0;
  properties.clear();
});

test('the measured scroll offset follows responsive header height changes', () => {
  header = makeHeader(410);
  syncPinnedHomeHeader();
  assert.equal(offset(), '410px');
  assert.equal(observers[0].target, header);
  header.height = 284;
  observers[0].callback();
  assert.equal(offset(), '284px');
});

test('DOM patches reusing the header do not create more resize observers', () => {
  header = makeHeader(410);
  syncPinnedHomeHeader();
  syncPinnedHomeHeader();
  assert.equal(observers.length, 1);
  assert.equal(observers[0].disconnected, undefined);
});

test('replacing or leaving the header disconnects stale observers and clears the offset', () => {
  header = makeHeader(410);
  syncPinnedHomeHeader();
  const previous = observers[0];
  header = makeHeader(300);
  syncPinnedHomeHeader();
  assert.equal(previous.disconnected, true);
  previous.callback();
  assert.equal(offset(), '300px');
  const current = observers[1];
  header = undefined;
  syncPinnedHomeHeader();
  assert.equal(current.disconnected, true);
  current.callback();
  assert.equal(offset(), undefined);
});
