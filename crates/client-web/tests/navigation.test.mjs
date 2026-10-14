import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), server: { middlewareMode: true, hmr: false, ws: false } });
after(() => server.close());
const { findSpatialTarget } = await server.ssrLoadModule('/src/app/spatialNavigation.ts');
const { gamepadNavigationActions, createGamepadNavigationReader } = await server.ssrLoadModule('/src/app/gamepadNavigation.ts');
const { bindGlobalInputHandlers, syncNavigationState, syncNavigationAfterRender } = await server.ssrLoadModule('/src/app/input.ts');
const rect = (x, y, width = 100, height = 140) => ({ left: x, top: y, right: x + width, bottom: y + height, width, height });

test('arrows follow the visible grid even when DOM order differs', () => {
  const origin = rect(120, 160);
  const candidates = [rect(0, 320), rect(240, 160), rect(120, 320), rect(0, 160), rect(120, 0)];
  for (const [direction, index] of [['right', 1], ['down', 2], ['left', 3], ['up', 4]]) {
    assert.equal(findSpatialTarget(origin, candidates, (entry) => entry, direction), candidates[index]);
  }
  assert.equal(findSpatialTarget(origin, [candidates[3]], (entry) => entry, 'right'), undefined);
});

test('different card heights do not turn a vertical move into a sideways jump', () => {
  const origin = rect(0, 0, 100, 170);
  const shortNeighbor = rect(120, 0, 100, 140);
  assert.equal(findSpatialTarget(origin, [shortNeighbor], (entry) => entry, 'up'), undefined);
  assert.equal(findSpatialTarget(origin, [shortNeighbor], (entry) => entry, 'right'), shortNeighbor);
});

test('aligned targets win over nearer diagonal controls', () => {
  const aligned = rect(240, 0);
  assert.equal(findSpatialTarget(rect(0, 0), [rect(110, 160), aligned], (entry) => entry, 'right'), aligned);
});

test('a horizontal grid edge cannot jump diagonally into another row', () => {
  assert.equal(findSpatialTarget(rect(120, 160), [rect(240, 0)], (entry) => entry, 'right'), undefined);
});

const pad = (buttons = [], axes = [0, 0], index = 0) => ({
  index, connected: true, axes, buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: buttons.includes(i) })),
});

test('controller dead zone, D-pad precedence, and diagonals produce predictable single moves', () => {
  assert.deepEqual(gamepadNavigationActions(pad([], [0.4, -0.4])), []);
  assert.deepEqual(gamepadNavigationActions(pad([], [0.9, -0.7])), ['right']);
  assert.deepEqual(gamepadNavigationActions(pad([12], [0.9, 0.7])), ['up']);
  assert.deepEqual(gamepadNavigationActions(pad([0, 1, 4, 5])), ['previous-section', 'next-section', 'activate', 'back']);
  assert.deepEqual(gamepadNavigationActions(pad([], [])), []);
});

test('held directions repeat after a delay while activation and bumpers fire once', () => {
  const read = createGamepadNavigationReader();
  const controller = pad([0, 5, 15]);
  assert.deepEqual(read([controller], 0, false), ['right', 'next-section', 'activate']);
  assert.deepEqual(read([controller], 399, false), []);
  assert.deepEqual(read([controller], 400, false), ['right']);
  assert.deepEqual(read([controller], 509, false), []);
  assert.deepEqual(read([controller], 510, false), ['right']);
  read([pad()], 520, false);
  assert.deepEqual(read([controller], 521, false), ['right', 'next-section', 'activate']);
});

test('suspension consumes presses and disconnects clear held controller state', () => {
  const read = createGamepadNavigationReader();
  assert.deepEqual(read([pad([0, 1, 5])], 0, true), []);
  assert.deepEqual(read([pad([0, 1, 5])], 1000, false), []);
  read([null], 1010, false);
  assert.deepEqual(read([pad([0, 1, 5])], 1020, false), ['next-section', 'activate', 'back']);
});

// Minimal layout fixture drives the real global handlers without a new DOM dependency.
class Control {
  constructor(section, name, bounds, kind = 'button') {
    this.section = section;
    this.name = name;
    this.bounds = bounds;
    this.kind = kind;
    this.visible = true;
    this.disabled = false;
    this.tabIndex = 0;
    this.isConnected = true;
    this.dataset = {};
    this.classes = new Set();
    this.classList = { add: (name) => this.classes.add(name), remove: (name) => this.classes.delete(name), contains: (name) => this.classes.has(name), toggle: (name, on) => on ? this.classes.add(name) : this.classes.delete(name) };
    this.clicks = 0;
  }
  matches(selector) {
    if (selector === ':disabled') return this.disabled;
    if (selector === '#search-input') return this.id === 'search-input';
    if (selector === '[data-shelf-scroll]') return this.kind === 'scroll';
    if (selector.startsWith('.media-card')) return this.kind === 'card';
    return true;
  }
  closest(selector) {
    if (selector === '[data-nav-section]') return this.section;
    if (selector === 'form') return this.form ?? null;
    if (selector === '#search-form') return this.form?.id === 'search-form' ? this.form : null;
    if (selector === '.search-popover') return this.searchPopover ?? null;
    if (selector === '.home-pinned-header') return this.pinnedHeader ?? this.section?.pinnedHeader ?? null;
    if (selector === '.item-scroll-content') return this.itemContent ?? this.section?.itemContent ?? null;
    if (selector === '.item-action-dock .detail-actions') return this.itemActionRow ?? this.section?.itemActionRow ?? null;
    if (selector === '[data-shelf-row]') return this.shelfRow ?? null;
    if (selector.startsWith('input,')) return this.kind === 'input' ? this : null;
    return null;
  }
  getBoundingClientRect() { return this.visible ? this.bounds : rect(0, 0, 0, 0); }
  focus() { document.activeElement = this; syncNavigationState(); }
  scrollIntoView() { this.scrolled = true; }
  click() { this.clicks += 1; }
}
class Section extends Control {
  constructor(id, bounds) { super(null, id, bounds); this.dataset = { navSection: id, navLabel: id }; this.controls = []; }
  querySelectorAll() { return this.controls; }
}

let sections;
let controllers;
let frame;
let backCount;
let playerOverlay;
let searchPopover;
const listeners = new Map();
const appState = { route: { page: 'home' }, activeTrailer: undefined, isPlayerOpen: false };
globalThis.addEventListener = (name, listener) => listeners.set(name, listener);
globalThis.requestAnimationFrame = (callback) => { frame = callback; };
globalThis.getComputedStyle = (element) => ({ visibility: element.visible ? 'visible' : 'hidden', display: 'block' });
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { getGamepads: () => controllers } });
globalThis.history = { back: () => { backCount += 1; } };
globalThis.document = {
  activeElement: null, hidden: false, body: { dataset: {} },
  querySelectorAll: (selector) => selector === '[data-nav-section]' ? sections : selector === '.home-navbar [data-home-tab]' ? sections.flatMap((section) => section.controls).filter((control) => control.dataset.homeTab) : sections.filter((section) => section.classes.has('nav-section-active')),
  querySelector: (selector) => selector === '.browse-workspace' ? {} : selector === '.player-overlay' && playerOverlay ? {} : selector === '.search-popover' ? searchPopover : null,
};
bindGlobalInputHandlers(appState);
const key = (value, extra = {}) => {
  const event = { key: value, target: document.activeElement, preventDefault() { this.defaultPrevented = true; }, ...extra };
  listeners.get('keydown')(event);
  return event;
};
const add = (section, name, x, y, kind = 'card') => {
  const control = new Control(section, name, rect(x, y), kind);
  section.controls.push(control);
  return control;
};

beforeEach(() => {
  appState.route = { page: 'home' };
  appState.isPlayerOpen = false;
  appState.activeTrailer = undefined;
  document.hidden = false;
  document.activeElement = null;
  document.body.dataset = {};
  controllers = [];
  backCount = 0;
  playerOverlay = false;
  searchPopover = undefined;
  sections = [new Section('libraries', rect(0, 0, 100, 600)), new Section('browse', rect(120, 0, 300, 140)), new Section('shelf', rect(120, 180, 300, 140))];
  add(sections[0], 'Home', 0, 0, 'button');
  add(sections[0], 'Movies', 0, 160, 'button');
  add(sections[1], 'Browse first', 120, 0);
  add(sections[1], 'Browse second', 240, 0);
  add(sections[2], 'Shelf first', 120, 180);
  add(sections[2], 'Shelf second', 240, 180);
  frame(0);
});

test('initial arrows focus the first content control and edges do not wrap', () => {
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse first');
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse second');
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse second');
});

test('vertical section movement starts at the first card and restores previously visited positions', () => {
  // Model the hero Open button at the far right, beyond the shelf cards.
  sections[1].controls[1].bounds = rect(900, 0);
  sections[1].controls[1].focus();
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf first');
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Shelf second');
  key('ArrowUp');
  assert.equal(document.activeElement.name, 'Browse second');
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf second');
  key('Tab', { shiftKey: true });
  assert.equal(document.activeElement.name, 'Browse second');
  key('Tab');
  assert.equal(document.activeElement.name, 'Shelf second');
  assert.equal(sections[2].classes.has('nav-section-active'), true);
  assert.equal(sections[1].classes.has('nav-section-active'), false);
  sections[1].controls[0].focus();
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf second');
});

test('moving up into an unvisited section starts at the first card', () => {
  sections[2].controls[1].focus();
  key('ArrowUp');
  assert.equal(document.activeElement.name, 'Browse first');
  key('ArrowRight');
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf second');
  key('ArrowUp');
  assert.equal(document.activeElement.name, 'Browse second');
});

test('returning to pinned hero controls does not scroll the media rows back to the top', () => {
  sections[1].pinnedHeader = {};
  sections[2].controls[0].focus();
  key('ArrowUp');
  assert.equal(document.activeElement.name, 'Browse first');
  assert.equal(document.activeElement.scrolled, undefined);
  assert.equal(sections[1].scrolled, undefined);
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf first');
  assert.equal(document.activeElement.scrolled, true);
});

test('item navigation reveals only the scrolling content and returning to playback preserves its position', () => {
  appState.route = { page: 'item', itemId: 101 };
  const content = { scrollTop: 300, getBoundingClientRect: () => rect(120, 180, 300, 140) };
  const actionRow = { scrollLeft: 0, getBoundingClientRect: () => rect(120, 500, 180, 100) };
  sections[1].itemActionRow = actionRow;
  sections[1].controls[0].bounds = rect(120, 500);
  sections[1].controls[1].bounds = rect(240, 500);
  sections[2].itemContent = content;
  sections[2].bounds = rect(120, 380, 300, 140);
  sections[2].controls[0].bounds = rect(120, 380);
  sections[2].controls[1].bounds = rect(240, 380);
  sections[1].controls[0].focus();
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Shelf first');
  assert.ok(content.scrollTop > 300);
  assert.equal(document.activeElement.scrolled, undefined);
  assert.equal(sections[2].scrolled, undefined);
  const position = content.scrollTop;
  key('ArrowUp');
  assert.equal(document.activeElement.name, 'Browse first');
  assert.equal(content.scrollTop, position);
  assert.equal(document.activeElement.scrolled, undefined);
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse second');
  assert.equal(actionRow.scrollLeft, 40);
  assert.equal(content.scrollTop, position);
});

test('search suggestions inside the pinned header can still scroll into view', () => {
  sections[1].pinnedHeader = {};
  sections[1].controls[1].searchPopover = {};
  sections[1].controls[0].focus();
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse second');
  assert.equal(document.activeElement.scrolled, true);
});

test('unavailable remembered cards fall back to the beginning of the row', () => {
  const remembered = sections[2].controls[1];
  for (const reason of ['hidden', 'disabled']) {
    remembered.visible = true;
    remembered.disabled = false;
    remembered.focus();
    sections[1].controls[1].focus();
    if (reason === 'hidden') remembered.visible = false;
    else remembered.disabled = true;
    key('ArrowDown');
    assert.equal(document.activeElement.name, 'Shelf first');
  }
});

test('controller row changes share the first-item and remembered-position behavior', () => {
  sections[1].controls[1].bounds = rect(900, 0);
  sections[1].controls[1].focus();
  controllers = [pad([13])]; frame(0);
  assert.equal(document.activeElement.name, 'Shelf first');
  controllers = [pad()]; frame(10);
  controllers = [pad([15])]; frame(20);
  assert.equal(document.activeElement.name, 'Shelf second');
  controllers = [pad()]; frame(30);
  controllers = [pad([12])]; frame(40);
  assert.equal(document.activeElement.name, 'Browse second');
  controllers = [pad()]; frame(50);
  controllers = [pad([13])]; frame(60);
  assert.equal(document.activeElement.name, 'Shelf second');
});

test('controller Left and Right change a selected filter without moving focus', () => {
  const select = add(sections[1], 'Sort', 120, 0, 'field');
  select.tagName = 'SELECT';
  select.options = [{}, {}];
  select.selectedIndex = 0;
  let changes = 0;
  select.dispatchEvent = (event) => { if (event.type === 'change') changes += 1; };
  select.focus();
  controllers = [pad([15])]; frame(0);
  assert.equal(select.selectedIndex, 1);
  assert.equal(document.activeElement, select);
  controllers = [pad()]; frame(10);
  controllers = [pad([15])]; frame(20);
  assert.equal(select.selectedIndex, 1);
  assert.equal(changes, 1);
  controllers = [pad()]; frame(30);
  controllers = [pad([14])]; frame(40);
  assert.equal(select.selectedIndex, 0);
  assert.equal(changes, 2);
});

test('sidebar movement and returning to content restore the previous control', () => {
  sections[2].controls[1].focus();
  key('ArrowLeft');
  key('ArrowLeft');
  assert.equal(document.activeElement.name, 'Home');
  key('ArrowDown');
  assert.equal(document.activeElement.name, 'Movies');
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Shelf first');
});

test('invisible and disabled controls are skipped, including whole empty sections', () => {
  sections[1].controls[1].disabled = true;
  sections[2].controls.forEach((control) => { control.visible = false; });
  sections[1].controls[0].focus();
  key('ArrowRight');
  assert.equal(document.activeElement.name, 'Browse first');
  key('Tab');
  assert.equal(document.activeElement.name, 'Home');
});

test('native editing and modified shortcuts are preserved while Tab switches sections from a field', () => {
  const input = add(sections[1], 'Search', 360, 0, 'input');
  input.focus();
  assert.equal(key('ArrowLeft').defaultPrevented, undefined);
  assert.equal(document.activeElement, input);
  assert.equal(key('ArrowRight', { ctrlKey: true }).defaultPrevented, undefined);
  key('Tab');
  assert.equal(document.activeElement.name, 'Shelf first');
});

test('Tab reaches all form fields before joining the next or previous section', () => {
  const input = add(sections[1], 'Search', 360, 0, 'input');
  const submit = add(sections[1], 'Submit search', 480, 0, 'button');
  const form = { querySelectorAll: () => [input, submit] };
  input.form = form;
  submit.form = form;
  input.focus();
  assert.equal(key('Tab').defaultPrevented, undefined);
  submit.focus(); // The browser's native Tab step.
  key('Tab');
  assert.equal(document.activeElement.name, 'Shelf first');
  input.focus();
  key('Tab', { shiftKey: true });
  assert.equal(document.activeElement.name, 'Home');
});

test('new page focus starts in content and never steals focus from a player', () => {
  document.body.dataset.inputMode = 'keyboard';
  syncNavigationAfterRender();
  assert.equal(document.activeElement.name, 'Browse first');
  document.activeElement = null;
  playerOverlay = true;
  syncNavigationAfterRender();
  assert.equal(document.activeElement, null);
  playerOverlay = false;
  document.body.dataset.inputMode = 'pointer';
  syncNavigationAfterRender();
  assert.equal(document.activeElement, null);
});

test('controller actions use the same section model and stop during settings, playback, or hidden tabs', () => {
  sections[1].controls[1].focus();
  controllers = [pad([5])];
  frame(0);
  assert.equal(document.activeElement.name, 'Shelf first');
  assert.equal(document.body.dataset.inputMode, 'gamepad');
  controllers = [pad()]; frame(10);
  for (const suspend of ['settings', 'player', 'trailer', 'hidden']) {
    appState.route = { page: suspend === 'settings' ? 'settings' : 'home' };
    appState.isPlayerOpen = suspend === 'player';
    appState.activeTrailer = suspend === 'trailer' ? {} : undefined;
    document.hidden = suspend === 'hidden';
    controllers = [pad([0, 1, 15])]; frame(20);
    assert.equal(document.activeElement.name, 'Shelf first');
    assert.equal(document.activeElement.clicks, 0);
    assert.equal(backCount, 0);
    assert.equal(key('ArrowRight').defaultPrevented, undefined);
    controllers = [pad()]; frame(30);
  }
});

test('mouse input clears section decoration without losing the remembered target', () => {
  sections[1].controls[1].focus();
  key('ArrowUp');
  assert.equal(sections[1].classes.has('nav-section-active'), true);
  listeners.get('pointerdown')();
  assert.equal(sections[1].classes.has('nav-section-active'), false);
  key('Tab'); key('Tab', { shiftKey: true });
  assert.equal(document.activeElement.name, 'Browse second');
});

test('opening a player stops other controller actions from the same frame', () => {
  const control = sections[1].controls[0];
  control.focus();
  control.click = () => { appState.isPlayerOpen = true; };
  controllers = [pad([0, 1])];
  frame(0);
  assert.equal(appState.isPlayerOpen, true);
  assert.equal(backCount, 0);
});

function searchFixture() {
  searchPopover = { hidden: false };
  const [firstTab, secondTab] = sections[1].controls;
  firstTab.dataset.homeTab = 'recommended';
  firstTab.classes.add('active');
  secondTab.dataset.homeTab = 'library';
  const search = add(sections[1], 'Search', 360, 0, 'input');
  search.id = 'search-input';
  search.value = 'Mock Movie';
  search.selectionStart = search.selectionEnd = search.value.length;
  const submit = add(sections[1], 'Search button', 480, 0, 'button');
  const form = { id: 'search-form', querySelectorAll: () => [search, submit] };
  search.form = form;
  submit.form = form;
  secondTab.focus();
  search.focus();
  return { firstTab, secondTab, search, submit };
}

test('Escape returns from search to the last focused browse tab without losing the query', () => {
  const { secondTab, search } = searchFixture();
  assert.equal(key('Escape').defaultPrevented, true);
  assert.equal(document.activeElement, secondTab);
  assert.equal(search.value, 'Mock Movie');
  assert.equal(searchPopover.hidden, true);
  assert.equal(backCount, 0);
});

test('Left edits the search text until its caret reaches the start, then returns to tabs', () => {
  const { secondTab, search } = searchFixture();
  assert.equal(key('ArrowLeft').defaultPrevented, undefined);
  assert.equal(document.activeElement, search);
  search.selectionStart = 0;
  search.selectionEnd = search.value.length;
  assert.equal(key('ArrowLeft').defaultPrevented, undefined);
  search.selectionEnd = 0;
  assert.equal(key('ArrowLeft').defaultPrevented, true);
  assert.equal(document.activeElement, secondTab);
});

test('controller Left and B leave search before B can navigate browser history', () => {
  const { secondTab, submit } = searchFixture();
  controllers = [pad([14])]; frame(0);
  assert.equal(document.activeElement, secondTab);
  controllers = [pad()]; frame(10);
  submit.focus();
  controllers = [pad([1])]; frame(20);
  assert.equal(document.activeElement, secondTab);
  assert.equal(backCount, 0);
  controllers = [pad()]; frame(30);
  controllers = [pad([1])]; frame(40);
  assert.equal(backCount, 1);
});

test('returning from search falls back to the active tab if the remembered tab is hidden', () => {
  const { firstTab, secondTab } = searchFixture();
  secondTab.visible = false;
  key('Escape');
  assert.equal(document.activeElement, firstTab);
});

test('suggestions stay available while navigating results and close when leaving search', () => {
  const { search, secondTab } = searchFixture();
  assert.equal(searchPopover.hidden, false);
  const preview = new Section('search-preview', rect(360, 160));
  sections.push(preview);
  const result = add(preview, 'Search result', 360, 160);
  result.searchPopover = searchPopover;
  result.focus();
  assert.equal(searchPopover.hidden, false);
  secondTab.focus();
  assert.equal(searchPopover.hidden, true);
  search.focus();
  assert.equal(searchPopover.hidden, false);
});
