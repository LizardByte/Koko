/** Verifies listening hold gestures, restart choices, and safe menu teardown without a media decoder. */
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
globalThis.location = { pathname: '/', origin: 'https://koko.local' };
globalThis.localStorage = { getItem: () => null };
const { bindAudiobookActions } = await server.ssrLoadModule('/src/app/audiobookActions.ts');
const { RESUME_HOLD_MS } = await server.ssrLoadModule('/src/app/resumeHold.ts');
const event = (type, detail = {}) => Object.assign(new Event(type, { cancelable: true }), detail);
const flush = () => new Promise((resolve) => setImmediate(resolve));

function fixture(t, fail = false) {
  const original = { document: globalThis.document, Node: globalThis.Node, addEventListener: globalThis.addEventListener };
  class Node extends EventTarget {
    constructor() { super(); this.dataset = {}; this.attributes = new Map(); this.classList = { add() {}, remove() {} }; }
    setAttribute(key, value) { this.attributes.set(key, value); }
    closest() { return null; }
    contains(node) { return this === node || this.children?.includes(node); }
    focus() { globalThis.document.activeElement = this; }
  }
  const trigger = new Node(), menu = new Node(), track = new Node(), book = new Node();
  trigger.dataset.listenBook = '500'; menu.hidden = true; menu.children = [track, book];
  track.dataset.restartAudiobook = 'track'; book.dataset.restartAudiobook = 'book';
  menu.querySelector = () => track; menu.querySelectorAll = () => [track, book];
  const doc = new EventTarget(), windowEvents = new EventTarget();
  doc.querySelector = (selector) => selector === '[data-listen-book]' ? trigger : menu;
  Object.assign(globalThis, { document: doc, Node, addEventListener: windowEvents.addEventListener.bind(windowEvents) });
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const calls = [], errors = [], controller = new AbortController();
  bindAudiobookActions(controller.signal, (error) => errors.push(error.message), {
    async listen(id) { calls.push(['listen', id]); },
    async restart(id, scope) { calls.push([scope, id]); if (fail) throw new Error('Unavailable narration'); },
  });
  t.after(() => { controller.abort(); Object.assign(globalThis, original); });
  return { trigger, menu, track, book, controller, calls, errors };
}

test('a short activation listens while a long hold opens both restart choices and consumes its release', async (t) => {
  const f = fixture(t);
  f.trigger.dispatchEvent(event('click')); await flush();
  assert.deepEqual(f.calls, [['listen', 500]]);
  f.trigger.dispatchEvent(event('pointerdown', { button: 0, clientX: 10, clientY: 10 }));
  t.mock.timers.tick(RESUME_HOLD_MS);
  assert.equal(f.menu.hidden, false); assert.equal(document.activeElement, f.track);
  f.trigger.dispatchEvent(event('pointerup')); f.trigger.dispatchEvent(event('click')); await flush();
  assert.equal(f.calls.length, 1);
  f.book.dispatchEvent(event('click')); await flush();
  assert.deepEqual(f.calls.at(-1), ['book', 500]); assert.equal(f.menu.hidden, true);
  f.trigger.dispatchEvent(event('contextmenu'));
  f.track.dispatchEvent(event('click')); await flush();
  assert.deepEqual(f.calls.at(-1), ['track', 500]);
});

test('keyboard and context menus dismiss to their own trigger and teardown removes action handlers', async (t) => {
  const f = fixture(t);
  f.trigger.dispatchEvent(event('keydown', { key: 'Enter' })); t.mock.timers.tick(RESUME_HOLD_MS);
  f.trigger.dispatchEvent(event('keyup', { key: 'Enter' }));
  assert.equal(f.menu.hidden, false); assert.equal(f.calls.length, 0);
  f.menu.dispatchEvent(event('keydown', { key: 'Escape' }));
  assert.equal(f.menu.hidden, true); assert.equal(document.activeElement, f.trigger);
  f.trigger.dispatchEvent(event('contextmenu'));
  f.trigger.dispatchEvent(event('resume-options-close')); assert.equal(f.menu.hidden, true);
  f.controller.abort(); f.trigger.dispatchEvent(event('click')); await flush();
  assert.equal(f.calls.length, 0);
});

test('restart failures restore the listening trigger and report the error without starting a different file', async (t) => {
  const f = fixture(t, true);
  f.trigger.dispatchEvent(event('contextmenu')); f.book.dispatchEvent(event('click'));
  assert.equal(f.trigger.disabled, true); await flush();
  assert.equal(f.trigger.disabled, false); assert.deepEqual(f.errors, ['Unavailable narration']);
  assert.deepEqual(f.calls, [['book', 500]]);
});
