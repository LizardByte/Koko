import assert from 'node:assert/strict';
import { after, afterEach, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

globalThis.location = { pathname: '/settings/libraries', origin: 'http://localhost' };
globalThis.localStorage = { getItem: () => null };
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { state } = await server.ssrLoadModule('/src/app/state.ts');
const { bindSettingsActions } = await server.ssrLoadModule('/src/app/settingsActions.ts');

let windowEvents, scroller, anchorTop, scrollerTop, bar, identity, observer, controller, styles;
let trigger, panel, cancel, input, focused, attributes, reveals;
let defaultActions, creationActions;

beforeEach(() => {
  state.settingsCreation = undefined;
  controller = new AbortController();
  windowEvents = new EventTarget();
  globalThis.addEventListener = windowEvents.addEventListener.bind(windowEvents);
  styles = new Map();
  scrollerTop = 0;
  anchorTop = 180;
  scroller = new EventTarget();
  scroller.getBoundingClientRect = () => ({ top: scrollerTop });
  scroller.style = { setProperty: (key, value) => styles.set(key, value), removeProperty: (key) => styles.delete(key) };
  identity = { hidden: true };
  defaultActions = { hidden: false };
  creationActions = { hidden: true };
  attributes = new Map([['aria-controls', 'add-library-panel'], ['aria-expanded', 'false']]);
  trigger = new EventTarget();
  trigger.dataset = { settingsCreate: 'libraries' };
  trigger.getAttribute = (key) => attributes.get(key);
  trigger.setAttribute = (key, value) => attributes.set(key, value);
  trigger.focus = () => { focused = trigger; };
  cancel = new EventTarget();
  input = { value: 'Pending library', focus: () => { focused = input; } };
  panel = new EventTarget();
  panel.hidden = true;
  panel.querySelector = (selector) => selector === 'input' ? input : cancel;
  reveals = 0;
  panel.scrollIntoView = () => { reveals += 1; };
  bar = {
    offsetHeight: 68, stuck: false, closest: () => scroller,
    querySelector: (selector) => ({
      '[data-sticky-identity]': identity, '[data-settings-default-actions]': defaultActions,
      '[data-settings-creation-actions]': creationActions, '[data-close-settings-create]': cancel,
    })[selector],
    querySelectorAll: () => [trigger],
    classList: { toggle: (_, value) => { bar.stuck = value; } },
  };
  globalThis.document = {
    querySelector: (selector) => selector === '.settings-action-dock' ? bar : { getBoundingClientRect: () => ({ top: anchorTop }) },
    getElementById: () => panel,
  };
  globalThis.ResizeObserver = class {
    constructor(callback) { this.callback = callback; observer = this; }
    observe() {}
    disconnect() { this.disconnected = true; }
  };
  focused = undefined;
  bindSettingsActions(controller.signal);
});
afterEach(() => controller.abort());

test('settings actions pin in the pane and document, measure wrapped controls, and release observers', () => {
  assert.equal(bar.stuck, false);
  assert.equal(identity.hidden, true);
  assert.equal(styles.get('--settings-action-height'), '84px');
  anchorTop = -10;
  scroller.dispatchEvent(new Event('scroll'));
  assert.equal(bar.stuck, true);
  assert.equal(identity.hidden, false);
  bar.offsetHeight = 116;
  observer.callback();
  assert.equal(styles.get('--settings-action-height'), '132px');
  // A phone scrolls the document rather than an inner settings pane.
  scrollerTop = -100;
  anchorTop = 5;
  windowEvents.dispatchEvent(new Event('scroll'));
  assert.equal(bar.stuck, false);
  anchorTop = -1;
  windowEvents.dispatchEvent(new Event('scroll'));
  assert.equal(bar.stuck, true);
  controller.abort();
  assert.equal(observer.disconnected, true);
  assert.equal(styles.has('--settings-action-height'), false);
  anchorTop = 180;
  windowEvents.dispatchEvent(new Event('scroll'));
  assert.equal(bar.stuck, true);
});

test('Add and Cancel manage focus and accessible state without discarding a draft', () => {
  trigger.dispatchEvent(new Event('click'));
  assert.equal(panel.hidden, false);
  assert.equal(state.settingsCreation, 'libraries');
  assert.equal(attributes.get('aria-expanded'), 'true');
  assert.equal(defaultActions.hidden, true);
  assert.equal(creationActions.hidden, false);
  assert.equal(focused, input);
  assert.equal(reveals, 1);
  cancel.dispatchEvent(new Event('click'));
  assert.equal(panel.hidden, true);
  assert.equal(state.settingsCreation, undefined);
  assert.equal(attributes.get('aria-expanded'), 'false');
  assert.equal(defaultActions.hidden, false);
  assert.equal(creationActions.hidden, true);
  assert.equal(focused, trigger);
  trigger.dispatchEvent(new Event('click'));
  assert.equal(input.value, 'Pending library');
  assert.equal(panel.hidden, false);
});

test('Escape returns to Add and render teardown removes creation handlers', () => {
  trigger.dispatchEvent(new Event('click'));
  const escape = new Event('keydown', { cancelable: true });
  escape.key = 'Escape';
  panel.dispatchEvent(escape);
  assert.equal(escape.defaultPrevented, true);
  assert.equal(panel.hidden, true);
  assert.equal(focused, trigger);
  controller.abort();
  trigger.dispatchEvent(new Event('click'));
  assert.equal(panel.hidden, true);
});
