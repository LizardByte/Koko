import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { bindItemPlaybackBar, bindItemOpeningLayout, itemCoverDimensions, completeRowCardCount } = await server.ssrLoadModule('/src/app/itemLayout.ts');

test('the playback bar adapts to scrolling and resizing, including a restored position, and stops after teardown', () => {
  const windowEvents = new EventTarget();
  globalThis.addEventListener = windowEvents.addEventListener.bind(windowEvents);
  const scroller = new EventTarget();
  scroller.getBoundingClientRect = () => ({ top: 40 });
  let anchorTop = -100; // Binding after a DOM patch must honor the existing scroll position.
  const anchor = { getBoundingClientRect: () => ({ top: anchorTop }) };
  const identity = { hidden: true };
  let stuck;
  const bar = { querySelector: () => identity, classList: { toggle: (_, value) => { stuck = value; } } };
  const controller = new AbortController();
  bindItemPlaybackBar(scroller, anchor, bar, controller.signal);
  assert.equal(stuck, true);
  assert.equal(identity.hidden, false);
  anchorTop = 200;
  scroller.dispatchEvent(new Event('scroll'));
  assert.equal(stuck, false);
  assert.equal(identity.hidden, true);
  anchorTop = 39;
  windowEvents.dispatchEvent(new Event('resize'));
  assert.equal(stuck, true);
  controller.abort();
  anchorTop = 200;
  scroller.dispatchEvent(new Event('scroll'));
  windowEvents.dispatchEvent(new Event('resize'));
  assert.equal(stuck, true);
});

test('the shared cover width and portrait backdrop reference are independent of opening content', () => {
  assert.deepEqual(itemCoverDimensions(1400), { width: 300, height: 450 });
  assert.deepEqual(itemCoverDimensions(1000), { width: 240, height: 360 });
  assert.deepEqual(itemCoverDimensions(600), { width: 144, height: 216 });
  assert.deepEqual(itemCoverDimensions(0), { width: 0, height: 0 });
});

test('wrapped descriptions get Show more, expansion leaves the cover stable, and observers are torn down', () => {
  let queued;
  let observer;
  let episodeLayout = false;
  globalThis.requestAnimationFrame = (callback) => { queued = callback; return 1; };
  globalThis.cancelAnimationFrame = () => { queued = undefined; };
  globalThis.ResizeObserver = class {
    constructor(callback) { this.callback = callback; observer = this; }
    observe() {}
    disconnect() { this.disconnected = true; }
  };
  const styles = new Map();
  const style = { getPropertyValue: (name) => styles.get(name) ?? '', setProperty: (name, value) => styles.set(name, value), removeProperty: (name) => styles.delete(name) };
  const text = { scrollHeight: 500, height: 144, getBoundingClientRect: () => ({ height: text.height }) };
  const toggle = { hidden: false };
  const description = { querySelector: (selector) => selector === '[data-measure-overflow]' ? text : toggle, getBoundingClientRect: () => ({ bottom: 300 + text.height + (toggle.hidden ? 0 : 56) }) };
  const identity = { querySelector: () => ({ getBoundingClientRect: () => ({ top: 20 }) }) };
  const scroller = { style, clientWidth: 1400, getBoundingClientRect: () => ({ top: 40, left: 88 }) };
  const page = { isConnected: true, clientWidth: 1400, style, classList: { contains: (name) => name === 'episode-detail-page' && episodeLayout }, closest: () => scroller, getBoundingClientRect: () => ({ top: 0 }), querySelectorAll: () => [], querySelector: (selector) => ({ '.item-identity': identity, '.item-description-section': description }[selector]) };
  globalThis.getComputedStyle = (element) => ({ lineHeight: '24px', paddingTop: '16px', getPropertyValue: () => element === page ? '1' : '' });
  const controller = new AbortController();
  bindItemOpeningLayout(page, controller.signal);
  // Dimensions must be present synchronously, before any animation-frame paint.
  assert.equal(toggle.hidden, false);
  assert.equal(styles.get('--item-cover-height'), '450px');
  assert.equal(styles.get('--item-artwork-height'), '642px');
  assert.equal(styles.get('--item-artwork-top'), '40px');
  assert.equal(styles.get('--item-artwork-left'), '88px');
  assert.equal(styles.get('--item-artwork-width'), '1400px');
  assert.equal(queued, undefined);
  text.height = 500;
  observer.callback(); queued();
  assert.equal(styles.get('--item-cover-height'), '450px');
  assert.equal(styles.get('--item-artwork-height'), '642px');
  text.height = text.scrollHeight = 72;
  observer.callback(); queued();
  assert.equal(toggle.hidden, true);
  assert.equal(styles.get('--item-cover-height'), '450px');
  assert.equal(styles.get('--item-artwork-height'), '642px');
  episodeLayout = true;
  observer.callback(); queued();
  assert.equal(styles.get('--item-cover-width'), '300px');
  assert.equal(styles.get('--item-cover-height'), '168.8px');
  assert.equal(styles.get('--item-artwork-height'), '642px');
  controller.abort();
  assert.equal(observer.disconnected, true);
});

test('row sizing fits whole people and extra cards across desktop and phone widths', () => {
  assert.equal(completeRowCardCount(1280, 135, 16), 8);
  assert.equal(completeRowCardCount(328, 120, 16), 2);
  assert.equal(completeRowCardCount(1280, 230, 16), 5);
  assert.equal(completeRowCardCount(328, 230, 16), 1);
  assert.equal(completeRowCardCount(100, 230, 16), 1);
});
