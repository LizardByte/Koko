/** Exercises whole-book pagination, layout changes, cancellation, and resource cleanup. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { SectionProgress } from 'foliate-js/progress.js';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { createReaderPagination, readerColumnPages, readerTextPageLabel, readerTextPagePreview, verifiedReaderPages } = await server.ssrLoadModule('/src/app/readerPagination.ts');
const { measureReaderTextPages } = await server.ssrLoadModule('/src/app/readerText.ts');
const { readerSequence } = await server.ssrLoadModule('/src/app/readerSupport.ts');
const layout = { width: 1000, height: 700, size: 100, direction: 'ltr', spread: false };
const flush = () => new Promise((resolve) => setImmediate(resolve));
const result = (total) => ({ total, sections: [total], columns: [1] });

test('reader sequences decode lazily in order and stop when their consumer closes', async () => {
  const started = [];
  let finish;
  const sequence = readerSequence([1, 2, 3], (item) => new Promise((resolve) => { started.push(item); finish = resolve; }));
  assert.deepEqual(started, []);
  const first = sequence.next();
  assert.deepEqual(started, [1]);
  finish('first');
  assert.deepEqual(await first, { value: 'first', done: false });
  assert.deepEqual(started, [1]);
  const second = sequence.next();
  assert.deepEqual(started, [1, 2]);
  finish('second');
  assert.deepEqual(await second, { value: 'second', done: false });
  await sequence.return();
  assert.deepEqual(started, [1, 2]);
});

test('pagination debounces resize and publishes only the most recent complete layout', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const jobs = [], counts = [];
  const pagination = createReaderPagination((layout, signal) => new Promise((resolve, reject) => jobs.push({ layout, signal, resolve, reject })), (count) => counts.push(count));
  t.after(() => pagination.close());
  pagination.schedule(layout); pagination.schedule({ ...layout, width: 900 });
  t.mock.timers.tick(250); assert.equal(jobs.length, 1); assert.equal(jobs[0].layout.width, 900);
  pagination.schedule({ ...layout, width: 900 }); t.mock.timers.tick(250); assert.equal(jobs.length, 1);
  pagination.schedule({ ...layout, size: 120 }); assert.equal(jobs[0].signal.aborted, true);
  t.mock.timers.tick(250); jobs[0].resolve(result(100)); jobs[1].resolve(result(160)); await flush();
  assert.deepEqual(counts, [result(160)]);
  pagination.schedule({ ...layout, spread: true }); t.mock.timers.tick(250);
  pagination.close(); jobs[2].resolve(result(80)); await flush(); assert.deepEqual(counts, [result(160)]);
  pagination.schedule(layout); t.mock.timers.tick(250); assert.equal(jobs.length, 3);
});

test('partial failures and invalid totals cannot erase a previously completed count', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const counts = [], results = [result(90), result(0), result(NaN), result(3.5), result(100001), { total: 90, sections: [1], columns: [1] }, new Error('Section unavailable'), result(110)];
  const pagination = createReaderPagination(async () => { const result = results.shift(); if (result instanceof Error) { throw result; } return result; }, (count) => counts.push(count));
  t.after(() => pagination.close());
  for (let size = 100; size < 108; size++) { pagination.schedule({ ...layout, size }); t.mock.timers.tick(250); await flush(); }
  assert.deepEqual(counts, [result(90), result(110)]);
});

test('column counts exclude blank rectos and empty last half-spreads in either writing direction', () => {
  assert.equal(readerColumnPages(420, 500, false), 1);
  assert.equal(readerColumnPages(920, 500, false), 2);
  assert.equal(readerColumnPages(1420, 500, true), 2);
  assert.equal(readerColumnPages(1000.1, 500, false), 2);
  assert.equal(readerColumnPages(0, 500, false), 1);
  for (const [extent, pitch] of [[NaN, 500], [200, 0], [Infinity, 500]]) assert.throws(() => readerColumnPages(extent, pitch));
});

test('page totals use actual navigation screens and validate two-page spreads', () => {
  assert.equal(verifiedReaderPages(41, 41, 1, false), 41);
  assert.equal(verifiedReaderPages(1, 0, 1, false), 0);
  assert.equal(verifiedReaderPages(41, 21, 2, true), 41);
  assert.equal(verifiedReaderPages(40, 21, 2, true), 40);
  assert.equal(verifiedReaderPages(41, 121, 1, false), 121);
  assert.throws(() => verifiedReaderPages(41, NaN, 1, false), /Page layout unavailable/);
  assert.throws(() => verifiedReaderPages(41, 20, 2, true));
});

test('reader labels use the calculated 41-page total independently of 121 byte-based locations', () => {
  const pages = { total: 41, sections: [1, 0, 5, 35] };
  assert.equal(readerTextPageLabel(pages, 0, 0, 1), 'Page 1 of 41');
  assert.equal(readerTextPageLabel(pages, 2, 0, 1), 'Page 2 of 41');
  assert.equal(readerTextPageLabel(pages, 2, 4, 1), 'Page 6 of 41');
  assert.equal(readerTextPageLabel(pages, 3, 0, 1), 'Page 7 of 41');
  assert.equal(readerTextPageLabel(pages, 3, 34, 1), 'Page 41 of 41');
  assert.equal(readerTextPageLabel(pages, 3, 100, 1), 'Page 41 of 41');
  assert.equal(readerTextPageLabel(pages, 1, 0, 1), undefined);
});

test('spread labels count the cover, both visible pages, and an unpaired final page', () => {
  const pages = { total: 6, sections: [6] };
  assert.equal(readerTextPageLabel(pages, 0, 0, 2), 'Page 1 of 6');
  assert.equal(readerTextPageLabel(pages, 0, 1, 2), 'Pages 2–3 of 6');
  assert.equal(readerTextPageLabel(pages, 0, 2, 2), 'Pages 4–5 of 6');
  assert.equal(readerTextPageLabel(pages, 0, 3, 2), 'Page 6 of 6');
  assert.equal(readerTextPageLabel({ total: 3, sections: [3] }, 0, 1, 2), 'Pages 2–3 of 3');
});

test('text hover destinations follow chapter weights instead of multiplying the page total by a percentage', () => {
  const progress = new SectionProgress([{ size: 100 }, { size: 500, linear: 'no' }, { size: 900 }, { size: 100 }], 1500, 1600);
  const pages = { total: 41, sections: [1, 0, 5, 35], columns: [1, 0, 1, 1] };
  const cases = [[0, 'Page 1 of 41'], [100 / 1100, 'Page 2 of 41'], [0.5, 'Page 4 of 41'], [999 / 1100, 'Page 6 of 41'], [1000 / 1100, 'Page 7 of 41'], [1, 'Page 41 of 41']];
  for (const [fraction, expected] of cases) assert.equal(readerTextPagePreview(pages, ...progress.getSection(fraction)), expected);
  assert.equal(readerTextPagePreview(pages, 1, 0.5), undefined);
  const resized = { total: 82, sections: [2, 0, 10, 70], columns: [1, 0, 1, 1] };
  assert.equal(readerTextPagePreview(resized, ...progress.getSection(0.5)), 'Page 8 of 82');
});

test('hover previews use each section column layout and preserve cover and final half-spreads', () => {
  const progress = new SectionProgress([{ size: 300 }, { size: 700 }], 1500, 1600);
  const pages = { total: 10, sections: [4, 6], columns: [1, 2] };
  for (const [fraction, expected] of [[0.1, 'Page 2 of 10'], [0.3, 'Page 5 of 10'], [0.65, 'Pages 8–9 of 10'], [1, 'Page 10 of 10']]) {
    assert.equal(readerTextPagePreview(pages, ...progress.getSection(fraction)), expected);
  }
  const cover = { total: 4, sections: [4], columns: [2] };
  assert.equal(readerTextPagePreview(cover, 0, 0), 'Page 1 of 4');
  assert.equal(readerTextPagePreview(cover, 0, 0.25), 'Pages 2–3 of 4');
  assert.equal(readerTextPagePreview(cover, 0, 0.75), 'Page 4 of 4');
});

function fixture(t, options = {}) {
  const original = { document: globalThis.document, requestAnimationFrame: globalThis.requestAnimationFrame };
  let removed = 0, destroyed = 0, released = 0;
  const visited = [], styles = [];
  const doc = {
    fonts: { ready: Promise.resolve() }, body: {},
    defaultView: { getComputedStyle: () => ({ writingMode: options.vertical ? 'vertical-rl' : 'horizontal-tb', columnWidth: '480px', columnGap: '20px' }) },
    documentElement: { getBoundingClientRect: () => ({ left: 0, right: 1000 }) },
    createRange: () => ({ selectNodeContents() {}, getBoundingClientRect: () => ({ left: options.rtl ? 80 : 0, right: 920, width: 920, height: 920 }) }),
  };
  let index;
  const renderer = {
    columnCount: 1, size: 500, pages: 4, isConnected: true, style: {}, setAttribute() {}, open() {},
    async goTo(target) { index = target.index; visited.push(index); await options.load?.(index); },
    setStyles: (css) => styles.push(css), getContents: () => options.missing ? [] : [{ index, doc }],
    destroy() { destroyed++; }, remove() { removed++; },
  };
  const host = { parentElement: { append: (element) => assert.equal(element, renderer) } };
  globalThis.document = { createElement: () => renderer };
  globalThis.requestAnimationFrame = (callback) => { queueMicrotask(callback); return 1; };
  t.after(() => Object.assign(globalThis, original));
  const createBook = async () => ({ sections: [{ id: 'a' }, { id: 'nav', linear: 'no' }, { id: 'b' }].map((entry) => ({ ...entry, load: async () => '' })), destroy() { released++; } });
  return { host, createBook, visited, styles, renderer, cleanup: () => ({ removed, destroyed, released }) };
}

test('whole-book measurement visits every linear section and releases its independent decoder', async (t) => {
  const f = fixture(t);
  assert.deepEqual(await measureReaderTextPages(f.createBook, f.host, layout, new AbortController().signal), { total: 4, sections: [2, 0, 2], columns: [1, 0, 1] });
  assert.deepEqual(f.visited, [0, 2]); assert.ok(f.styles.every((css) => css.includes('font-size:100%')));
  assert.deepEqual(f.cleanup(), { removed: 1, destroyed: 1, released: 1 });
});

test('whole-book measurement retains each section actual column count for mixed-layout seek previews', async (t) => {
  const f = fixture(t, { load: (index) => { f.renderer.columnCount = index === 0 ? 2 : 1; } });
  const pages = await measureReaderTextPages(f.createBook, f.host, { ...layout, spread: true }, new AbortController().signal);
  assert.deepEqual(pages, { total: 5, sections: [3, 0, 2], columns: [2, 0, 1] });
  assert.equal(readerTextPagePreview(pages, 0, 1), 'Pages 2–3 of 5');
  assert.equal(readerTextPagePreview(pages, 2, 1), 'Page 5 of 5');
});

test('vertical and right-to-left measurements use their respective column geometry', async (t) => {
  for (const options of [{ vertical: true }, { rtl: true }]) {
    const f = fixture(t, options);
    assert.deepEqual(await measureReaderTextPages(f.createBook, f.host, { ...layout, direction: 'rtl' }, new AbortController().signal), { total: 4, sections: [2, 0, 2], columns: [1, 0, 1] });
  }
});

test('failed or canceled sections release measurement resources without returning a partial count', async (t) => {
  const controller = new AbortController();
  let finish;
  const f = fixture(t, { load: (index) => {
    if (index !== 2) return;
    controller.abort();
    return new Promise((resolve) => { finish = resolve; });
  } });
  await assert.rejects(measureReaderTextPages(f.createBook, f.host, layout, controller.signal));
  assert.deepEqual(f.visited, [0, 2]);
  assert.deepEqual(f.cleanup(), { removed: 1, destroyed: 1, released: 1 });
  finish();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(f.cleanup(), { removed: 1, destroyed: 2, released: 1 });
  const missing = fixture(t, { missing: true });
  await assert.rejects(measureReaderTextPages(missing.createBook, missing.host, layout, new AbortController().signal), /Page layout unavailable/);
  assert.deepEqual(missing.cleanup(), { removed: 1, destroyed: 1, released: 1 });
});

test('an unresponsive measurement frame times out and still releases its resources', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let finish;
  const f = fixture(t, { load: () => new Promise((resolve) => { finish = resolve; }) });
  const pending = measureReaderTextPages(f.createBook, f.host, layout, new AbortController().signal);
  const rejected = assert.rejects(pending, /Pagination timed out/);
  await flush(); t.mock.timers.tick(15000); await rejected;
  assert.deepEqual(f.cleanup(), { removed: 1, destroyed: 1, released: 1 });
  finish(); await flush();
  assert.deepEqual(f.cleanup(), { removed: 1, destroyed: 2, released: 1 });
});
