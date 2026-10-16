import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { createRouteRenderer } = await server.ssrLoadModule('/src/app/routeRenderer.ts');
let frames;
beforeEach(() => {
  frames = new Map();
  let sequence = 0;
  globalThis.requestAnimationFrame = (callback) => { frames.set(++sequence, callback); return sequence; };
  globalThis.cancelAnimationFrame = (id) => frames.delete(id);
});
const flush = () => { const pending = [...frames.values()]; frames.clear(); pending.forEach((callback) => callback()); };

test('fast item navigation paints its content directly without flashing a loading page', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let ready = false;
  const paints = [];
  const route = createRouteRenderer({ initial: 'deferred', isCurrent: () => true, isReady: () => ready, render: (preserve) => paints.push({ ready, preserve }) });
  route.publish(); // Capabilities and settings arrived before the item.
  t.mock.timers.tick(100);
  assert.deepEqual(paints, []);
  assert.equal(frames.size, 0);
  ready = true;
  route.publish();
  route.finish(); // A fast final response can arrive before the render frame.
  flush();
  t.mock.timers.tick(1000);
  assert.deepEqual(paints, [{ ready: true, preserve: false }]);
});

test('slow item navigation shows loading once, then coalesces ready updates without replacing the page again', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let ready = false;
  const paints = [];
  const route = createRouteRenderer({ initial: 'deferred', isCurrent: () => true, isReady: () => ready, render: (preserve) => paints.push({ ready, preserve }) });
  t.mock.timers.tick(149);
  assert.deepEqual(paints, []);
  t.mock.timers.tick(1);
  assert.deepEqual(paints, [{ ready: false, preserve: false }]);
  ready = true;
  route.publish(); route.publish(); route.publish();
  assert.equal(frames.size, 1);
  flush();
  assert.deepEqual(paints[1], { ready: true, preserve: true });
  route.finish();
});

test('an abandoned route cannot paint its deferred loading page or late replies', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let current = true;
  const paints = [];
  const route = createRouteRenderer({ initial: 'deferred', isCurrent: () => current, isReady: () => true, render: (preserve) => paints.push(preserve) });
  current = false;
  t.mock.timers.tick(150);
  route.publish(); route.finish(); flush();
  assert.deepEqual(paints, []);
});

test('cold loads render immediately and background refreshes preserve their existing DOM', () => {
  for (const initial of ['immediate', 'none']) {
    const paints = [];
    const route = createRouteRenderer({ initial, isCurrent: () => true, isReady: () => true, render: (preserve) => paints.push(preserve) });
    assert.deepEqual(paints, initial === 'immediate' ? [false] : []);
    route.publish(); flush();
    assert.equal(paints.at(-1), true);
    route.finish();
  }
});
