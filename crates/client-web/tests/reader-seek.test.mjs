/** Exercises native slider events against delayed, quantized renderer destinations. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { bindReaderSeek } = await server.ssrLoadModule('/src/app/readerSeek.ts');
const event = (name, detail = {}) => Object.assign(new Event(name), detail);
const flush = () => new Promise((resolve) => setImmediate(resolve));

function fixture(t, predict = () => undefined) {
  const original = { window: globalThis.window, getComputedStyle: globalThis.getComputedStyle };
  const windowEvents = new EventTarget();
  globalThis.window = windowEvents; globalThis.getComputedStyle = () => ({ getPropertyValue: () => '12px' });
  t.after(() => Object.assign(globalThis, original));
  const range = new EventTarget(), preview = { hidden: true, textContent: '', style: { setProperty() {} } };
  const attributes = new Map(), pending = [], requests = [];
  Object.assign(range, { value: '0', max: '1000', style: { setProperty() {} }, setAttribute: (key, value) => attributes.set(key, value), getBoundingClientRect: () => ({ left: 100, width: 1012 }) });
  const controller = new AbortController();
  const report = bindReaderSeek(range, preview, { preview: predict, seek: (fraction) => new Promise((resolve, reject) => { requests.push(fraction); pending.push({ resolve, reject }); }) }, controller.signal);
  report(0.02, 'Location 1 of 121', 0);
  const input = (value) => { range.value = String(value); range.dispatchEvent(event('input')); };
  const finish = async (fraction, label, position = fraction) => { report(fraction, label, position); pending.shift().resolve(); await flush(); };
  t.after(() => controller.abort());
  return { range, preview, controller, report, input, finish, requests, pending, attributes, windowEvents };
}

test('drag previews report the actual rendered location and release cannot seek a second offset', async (t) => {
  const f = fixture(t);
  f.range.dispatchEvent(event('pointerdown'));
  f.input(500);
  assert.equal(f.preview.textContent, 'Opening location…');
  // A real EPUB renderer may snap 50% to the start of a visible page rather than a virtual location.
  await f.finish(0.53, 'Location 58 of 121', 0.48);
  assert.equal(f.preview.textContent, 'Location 58 of 121');
  assert.equal(f.range.value, '500');
  f.range.dispatchEvent(event('pointermove', { clientX: 900 }));
  assert.equal(f.preview.textContent, 'Location 58 of 121');
  f.windowEvents.dispatchEvent(event('pointerup'));
  f.range.dispatchEvent(event('change'));
  assert.equal(f.range.value, '480');
  assert.equal(f.attributes.get('aria-valuetext'), 'Location 58 of 121');
  assert.deepEqual(f.requests, [0.5]);
});

test('rapid scrubbing skips obsolete queued destinations and settles on the last native range value', async (t) => {
  const f = fixture(t, (fraction) => `Page ${Math.round(fraction * 6) + 1} of 7`);
  f.range.dispatchEvent(event('pointerdown'));
  f.input(100); f.input(250); f.input(800);
  assert.deepEqual(f.requests, [0.1]);
  assert.equal(f.preview.textContent, 'Page 6 of 7');
  await f.finish(2 / 7, 'Page 2 of 7', 1 / 6);
  assert.deepEqual(f.requests, [0.1, 0.8]);
  assert.equal(f.preview.textContent, 'Page 6 of 7');
  f.windowEvents.dispatchEvent(event('pointerup'));
  await f.finish(6 / 7, 'Page 6 of 7', 5 / 6);
  assert.equal(f.preview.textContent, 'Page 6 of 7');
  assert.equal(f.range.value, '833');
  assert.equal(f.attributes.get('aria-valuetext'), 'Page 6 of 7');
});

test('hover respects the native thumb travel and keyboard seeking does not duplicate change events', async (t) => {
  const fractions = [], f = fixture(t, (fraction) => { fractions.push(fraction); return `Target ${fraction}`; });
  for (const clientX of [106, 606, 1106]) f.range.dispatchEvent(event('pointermove', { clientX }));
  assert.deepEqual(fractions, [0, 0.5, 1]);
  f.input(600); f.range.dispatchEvent(event('change'));
  await f.finish(0.65, 'Location 70 of 121', 0.59);
  assert.deepEqual(f.requests, [0.6]);
  assert.equal(f.preview.textContent, 'Location 70 of 121');
  assert.equal(f.range.value, '590');
});

test('closing or canceling a scrub does not resurrect its preview or drain stale destinations', async (t) => {
  const f = fixture(t);
  f.range.dispatchEvent(event('pointerdown')); f.input(500); f.input(900);
  f.range.dispatchEvent(event('pointercancel'));
  assert.equal(f.preview.hidden, true);
  f.controller.abort();
  await f.finish(0.53, 'Location 58 of 121', 0.48);
  assert.equal(f.preview.hidden, true); assert.deepEqual(f.requests, [0.5]);
});
