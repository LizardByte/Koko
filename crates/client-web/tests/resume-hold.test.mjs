import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { bindResumeHold, RESUME_HOLD_MS } = await server.ssrLoadModule('/src/app/resumeHold.ts');

const event = (type, values = {}) => Object.assign(new Event(type, { cancelable: true }), values);
function fixture(t) {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const windowEvents = new EventTarget();
  globalThis.addEventListener = windowEvents.addEventListener.bind(windowEvents);
  const button = new EventTarget();
  const classes = new Set();
  const hint = { textContent: 'Hold for playback options' };
  button.classList = { add: (...names) => names.forEach((name) => classes.add(name)), remove: (...names) => names.forEach((name) => classes.delete(name)) };
  button.closest = () => ({ querySelector: () => hint });
  button.click = () => button.dispatchEvent(event('click'));
  const controller = new AbortController();
  let resumes = 0;
  let menus = 0;
  bindResumeHold(button, () => { menus += 1; }, controller.signal);
  button.addEventListener('click', () => { resumes += 1; });
  return { button, classes, hint, controller, windowEvents, counts: () => ({ resumes, menus }) };
}

test('short mouse and touch activations resume, while a deliberate hold opens options without playing', (t) => {
  const f = fixture(t);
  for (const pointerType of ['mouse', 'touch']) {
    f.button.dispatchEvent(event('pointerdown', { button: 0, clientX: 10, clientY: 10, pointerType }));
    t.mock.timers.tick(100);
    f.button.dispatchEvent(event('pointerup'));
    f.button.click();
  }
  assert.deepEqual(f.counts(), { resumes: 2, menus: 0 });
  f.button.dispatchEvent(event('pointerdown', { button: 0, clientX: 10, clientY: 10 }));
  t.mock.timers.tick(RESUME_HOLD_MS);
  assert.equal(f.classes.has('is-hold-ready'), true);
  assert.deepEqual(f.counts(), { resumes: 2, menus: 1 });
  f.button.dispatchEvent(event('pointerup'));
  f.button.click();
  assert.deepEqual(f.counts(), { resumes: 2, menus: 1 });
});

test('opening options mid-hold blocks key repeat and release on the newly focused option', (t) => {
  const f = fixture(t);
  f.button.dispatchEvent(event('keydown', { key: 'Enter' }));
  t.mock.timers.tick(RESUME_HOLD_MS);
  assert.deepEqual(f.counts(), { resumes: 0, menus: 1 });
  f.button.dispatchEvent(event('blur'));
  const repeat = event('keydown', { key: 'Enter', repeat: true });
  f.windowEvents.dispatchEvent(repeat);
  assert.equal(repeat.defaultPrevented, true);
  const release = event('keyup', { key: 'Enter' });
  f.windowEvents.dispatchEvent(release);
  assert.equal(release.defaultPrevented, true);
  assert.deepEqual(f.counts(), { resumes: 0, menus: 1 });
});

test('Enter and Space hold for options, consume native clicks, and ignore key repeat', (t) => {
  const f = fixture(t);
  for (const key of ['Enter', ' ']) {
    const down = event('keydown', { key });
    f.button.dispatchEvent(down);
    assert.equal(down.defaultPrevented, true);
    t.mock.timers.tick(100);
    f.button.dispatchEvent(event('keyup', { key }));
    f.button.dispatchEvent(event('keydown', { key }));
    t.mock.timers.tick(400);
    f.button.dispatchEvent(event('keydown', { key, repeat: true }));
    t.mock.timers.tick(RESUME_HOLD_MS - 400);
    f.button.dispatchEvent(event('keyup', { key }));
  }
  assert.deepEqual(f.counts(), { resumes: 2, menus: 2 });
});

test('gamepad holds use the same short and long activation semantics', (t) => {
  const f = fixture(t);
  f.button.dispatchEvent(event('resume-hold-start'));
  t.mock.timers.tick(100);
  f.button.dispatchEvent(event('resume-hold-end'));
  assert.deepEqual(f.counts(), { resumes: 1, menus: 0 });
  f.button.dispatchEvent(event('resume-hold-start'));
  t.mock.timers.tick(RESUME_HOLD_MS);
  f.button.dispatchEvent(event('resume-hold-end'));
  assert.deepEqual(f.counts(), { resumes: 1, menus: 1 });
});

test('scroll gestures and pointer cancellation suppress accidental playback', (t) => {
  const f = fixture(t);
  f.button.dispatchEvent(event('pointerdown', { button: 0, clientX: 10, clientY: 10 }));
  f.button.dispatchEvent(event('pointermove', { clientX: 10, clientY: 40 }));
  t.mock.timers.tick(RESUME_HOLD_MS);
  f.button.dispatchEvent(event('pointerup'));
  f.button.click();
  assert.deepEqual(f.counts(), { resumes: 0, menus: 0 });
  f.button.dispatchEvent(event('pointerdown', { button: 0, clientX: 10, clientY: 10 }));
  f.button.dispatchEvent(event('pointercancel'));
  f.button.click();
  assert.deepEqual(f.counts(), { resumes: 0, menus: 0 });
});

test('focus loss, controller cancellation, and render teardown cancel held input', (t) => {
  const f = fixture(t);
  for (const cancel of ['blur', 'resume-hold-cancel']) {
    f.button.dispatchEvent(event('resume-hold-start'));
    f.button.dispatchEvent(event(cancel));
    t.mock.timers.tick(RESUME_HOLD_MS);
    f.button.dispatchEvent(event('resume-hold-end'));
  }
  f.button.dispatchEvent(event('resume-hold-start'));
  f.controller.abort();
  t.mock.timers.tick(RESUME_HOLD_MS);
  assert.deepEqual(f.counts(), { resumes: 0, menus: 0 });
  assert.equal(f.classes.size, 0);
});
