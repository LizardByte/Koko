import type { AppState } from './types';
import { createGamepadNavigationReader } from './gamepadNavigation';
import { findSpatialTarget, type NavigationDirection } from './spatialNavigation';

const SECTION_SELECTOR = '[data-nav-section]';
const CONTROL_SELECTOR = 'button:not(:disabled), a[href], summary, input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';
const CARD_SELECTOR = '.media-card, .person-card, .media-extra-card, .filter-card-button, .search-result-row';
const rememberedFocus = new Map<string, HTMLElement>();
let lastContentSection: string | undefined;
let lastBrowseTab: string | undefined;

function isVisible(element: HTMLElement): boolean {
  if (element.closest('[hidden], [inert], [aria-hidden="true"]') || element.matches(':disabled')) return false;
  const rect = element.getBoundingClientRect();
  const style = getComputedStyle(element);
  return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
}

function sectionControls(section: HTMLElement): HTMLElement[] {
  return Array.from(section.querySelectorAll<HTMLElement>(CONTROL_SELECTOR))
    .filter((element) => element.closest(SECTION_SELECTOR) === section && isVisible(element));
}

function navigationSections(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(SECTION_SELECTOR))
    .filter((section) => isVisible(section) && sectionControls(section).length > 0);
}

function directionalControls(section: HTMLElement): HTMLElement[] {
  // Shelf arrows remain in the Tab order; directional input scrolls the cards itself.
  return sectionControls(section).filter((element) => !element.matches('[data-shelf-scroll]'));
}

function entryControl(section: HTMLElement | undefined): HTMLElement | undefined {
  if (!section) return undefined;
  const controls = directionalControls(section);
  const remembered = rememberedFocus.get(section.dataset.navSection!);
  if (remembered && controls.includes(remembered)) return remembered;
  const cards = controls.filter((control) => control.matches(CARD_SELECTOR));
  const targets = cards.length ? cards : controls;
  return targets[0];
}

function focusControl(control: HTMLElement | undefined, revealSection = false): void {
  if (!control) return;
  control.focus({ preventScroll: true });
  const section = control.closest<HTMLElement>(SECTION_SELECTOR);
  if (revealSection && section?.dataset.navSection !== 'libraries') {
    section?.scrollIntoView({ block: 'start', inline: 'nearest', behavior: 'instant' });
  }
  control.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  syncNavigationState();
}

function setInputMode(mode: 'pointer' | 'keyboard' | 'gamepad'): void {
  if (document.body.dataset.inputMode === mode) return;
  document.body.dataset.inputMode = mode;
  syncNavigationState();
}

/** Refresh section feedback after a DOM patch without changing the user's focus. */
export function syncNavigationState(): void {
  const active = document.activeElement as HTMLElement | null;
  const section = active?.closest<HTMLElement>(SECTION_SELECTOR);
  const mode = document.body.dataset.inputMode;
  const showFocus = mode === 'keyboard' || mode === 'gamepad';
  document.querySelectorAll<HTMLElement>('.nav-section-active').forEach((element) => {
    if (element !== section || !showFocus) element.classList.remove('nav-section-active');
  });
  if (section && active) {
    rememberedFocus.set(section.dataset.navSection!, active);
    if (section.dataset.navSection !== 'libraries') lastContentSection = section.dataset.navSection;
    section.classList.toggle('nav-section-active', showFocus);
    if (active.dataset.homeTab) lastBrowseTab = active.dataset.homeTab;
  }
  const inSearch = Boolean(active?.closest('#search-form'));
  const suggestions = document.querySelector<HTMLElement>('.search-popover');
  if (suggestions) suggestions.hidden = !inSearch && !active?.closest('.search-popover');
  const searchHint = document.querySelector<HTMLElement>('[data-search-navigation-hint]');
  if (searchHint) searchHint.hidden = !inSearch;
  const backLabel = document.querySelector<HTMLElement>('[data-gamepad-back-label]');
  if (backLabel) backLabel.textContent = inSearch ? 'Browse tabs' : 'Back';
  const label = document.querySelector<HTMLElement>('[data-navigation-section-label]');
  const text = section?.dataset.navLabel ?? 'Browse';
  if (label && label.textContent !== text) label.textContent = text;
}

/** Leave the search controls without discarding the query or changing pages. */
function returnToBrowseTabs(): boolean {
  if (!(document.activeElement as HTMLElement | null)?.closest('#search-form')) return false;
  const tabs = Array.from(document.querySelectorAll<HTMLElement>('.home-navbar [data-home-tab]')).filter(isVisible);
  const target = tabs.find((tab) => tab.dataset.homeTab === lastBrowseTab)
    ?? tabs.find((tab) => tab.classList.contains('active'))
    ?? tabs[0];
  if (!target) return false;
  focusControl(target);
  return true;
}

/** Restore visible feedback and give a newly opened page a useful starting control. */
export function syncNavigationAfterRender(): void {
  for (const [key, control] of rememberedFocus) {
    if (!control.isConnected) rememberedFocus.delete(key);
  }
  syncNavigationState();
  const mode = document.body.dataset.inputMode;
  if (!document.querySelector('.browse-workspace') || document.querySelector('.player-overlay') || !['keyboard', 'gamepad'].includes(mode ?? '')) return;
  if ((document.activeElement as HTMLElement | null)?.closest(SECTION_SELECTOR)) return;
  const sections = navigationSections();
  const overview = sections.find((section) => ['item-overview', 'person-overview', 'overview'].includes(section.dataset.navSection!));
  focusControl(entryControl(overview ?? sections.find((section) => section.dataset.navSection !== 'libraries')), true);
}

function moveSection(direction: 1 | -1): void {
  const sections = navigationSections();
  if (!sections.length) return;
  const current = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>(SECTION_SELECTOR);
  const index = current ? sections.indexOf(current) : -1;
  const firstSection = direction > 0 ? 0 : sections.length - 1;
  const next = index < 0 ? firstSection : (index + direction + sections.length) % sections.length;
  focusControl(entryControl(sections[next]), true);
}

function moveFocus(direction: NavigationDirection): void {
  const sections = navigationSections();
  const active = document.activeElement as HTMLElement | null;
  if (direction === 'left' && active?.matches('#search-input') && returnToBrowseTabs()) return;
  const section = active?.closest<HTMLElement>(SECTION_SELECTOR);
  if (!section || !active || !isVisible(active)) {
    focusControl(entryControl(sections.find((entry) => entry.dataset.navSection !== 'libraries') ?? sections[0]));
    return;
  }
  const rect = active.getBoundingClientRect();
  const next = findSpatialTarget(rect, directionalControls(section).filter((control) => control !== active), (control) => control.getBoundingClientRect(), direction);
  if (next) {
    focusControl(next);
    return;
  }
  const libraries = sections.find((entry) => entry.dataset.navSection === 'libraries');
  if (direction === 'left' && libraries && section !== libraries) {
    focusControl(entryControl(libraries));
  } else if (direction === 'right' && section === libraries) {
    const content = sections.find((entry) => entry.dataset.navSection === lastContentSection)
      ?? sections.find((entry) => entry !== libraries);
    focusControl(entryControl(content));
  } else if (direction === 'up' || direction === 'down') {
    const content = sections.filter((entry) => entry !== libraries);
    const index = content.indexOf(section);
    const adjacent = content[index + (direction === 'down' ? 1 : -1)];
    if (index >= 0 && adjacent) focusControl(entryControl(adjacent), true);
  }
}

function activateFocusedElement(): void {
  const focused = document.activeElement as HTMLElement | null;
  if (focused && isVisible(focused) && focused.matches(CONTROL_SELECTOR)) focused.click();
}

/** Return to browse tabs while keeping normal search-caret movement available. */
function handleSearchKey(event: KeyboardEvent, target: HTMLElement | null): boolean {
  if (event.key === 'Escape' && returnToBrowseTabs()) {
    event.preventDefault();
    return true;
  }
  if (event.key === 'ArrowLeft' && target?.matches('#search-input')) {
    const search = target as HTMLInputElement;
    if (search.selectionStart === 0 && search.selectionEnd === 0 && returnToBrowseTabs()) {
      event.preventDefault();
      return true;
    }
  }
  return false;
}

/** Keep native Tab movement between form fields until a form edge is reached. */
function keepsNativeFormTab(target: HTMLElement | null, reverse: boolean): boolean {
  const form = target?.closest('form');
  if (!form) return false;
  const fields = Array.from(form.querySelectorAll<HTMLElement>(CONTROL_SELECTOR)).filter((field) => isVisible(field) && field.tabIndex >= 0);
  const index = fields.indexOf(target!);
  if (index < 0) return false;
  return reverse ? index > 0 : index < fields.length - 1;
}

/** Handle section switching, including native field tabbing within a section. */
function handleSectionTab(event: KeyboardEvent, target: HTMLElement | null): boolean {
  if (event.key !== 'Tab' || !navigationSections().length) return false;
  if (keepsNativeFormTab(target, event.shiftKey)) return true;
  moveSection(event.shiftKey ? -1 : 1);
  event.preventDefault();
  return true;
}

/** Dispatch navigation keys after player and browser-shortcut guards have passed. */
function handleNavigationKey(event: KeyboardEvent): void {
  setInputMode('keyboard');
  const target = event.target as HTMLElement | null;
  if (handleSearchKey(event, target) || handleSectionTab(event, target)) return;
  if (target?.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])')) return;
  const directions: Record<string, NavigationDirection> = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' };
  if (directions[event.key] && navigationSections().length) {
    moveFocus(directions[event.key]);
    event.preventDefault();
  }
}

/** Bind section-aware navigation for browsing pages, leaving settings and players to their own controls. */
export function bindGlobalInputHandlers(state: Pick<AppState, 'activeTrailer' | 'isPlayerOpen' | 'route'>): void {
  const suspended = () => state.isPlayerOpen || Boolean(state.activeTrailer) || state.route.page === 'settings' || document.hidden;
  globalThis.addEventListener('pointermove', (event) => {
    if (event.movementX || event.movementY) setInputMode('pointer');
  }, { passive: true });
  globalThis.addEventListener('pointerdown', () => setInputMode('pointer'), { passive: true });
  globalThis.addEventListener('focusin', syncNavigationState);
  globalThis.addEventListener('keydown', (event) => {
    if (suspended() || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
    handleNavigationKey(event);
  });

  const readActions = createGamepadNavigationReader();
  const poll = (now: number) => {
    const actions = readActions(navigator.getGamepads?.() ?? [], now, suspended());
    if (actions.length) setInputMode('gamepad');
    for (const action of actions) {
      if (suspended()) break;
      if (action === 'previous-section' || action === 'next-section') moveSection(action === 'next-section' ? 1 : -1);
      else if (action === 'activate') activateFocusedElement();
      else if (action === 'back') {
        if (!returnToBrowseTabs()) globalThis.history.back();
      }
      else moveFocus(action);
    }
    globalThis.requestAnimationFrame(poll);
  };
  globalThis.requestAnimationFrame(poll);
}
