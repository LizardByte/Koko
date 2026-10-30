/** Coordinates keyboard and gamepad navigation, including held actions and modal reader isolation. */
import type { AppState } from './types';
import { createGamepadNavigationReader, type GamepadNavigationAction } from './gamepadNavigation';
import { findSpatialTarget, type NavigationDirection } from './spatialNavigation';

const SECTION_SELECTOR = '[data-nav-section]';
const CONTROL_SELECTOR = 'button:not(:disabled), a[href], summary, input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';
const CARD_SELECTOR = '.media-card, .episode-list-card, .person-card, .media-extra-card, .filter-card-button, .search-result-row';
const rememberedFocus = new Map<string, HTMLElement>();
let lastContentSection: string | undefined;
let lastBrowseTab: string | undefined;

function isVisible(element: HTMLElement): boolean {
  if (element.closest('[hidden], [inert], [aria-hidden="true"]') || element.matches(':disabled')) return false;
  // Closed details can retain layout boxes for their inaccessible descendants.
  // Only the first summary remains a navigation target, including in nested menus.
  let closedDetails = element.parentElement?.closest<HTMLDetailsElement>('details:not([open])');
  while (closedDetails) {
    if (!closedDetails.querySelector(':scope > summary')?.contains(element)) return false;
    closedDetails = closedDetails.parentElement?.closest<HTMLDetailsElement>('details:not([open])');
  }
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

/** Reveal item content below the sticky playback bar without resetting visible actions. */
function revealItemControl(control: HTMLElement, section: HTMLElement | null, revealSection: boolean): boolean {
  if (!control.closest('.item-detail-page')) return false;
  const scroller = control.closest<HTMLElement>('.main-shell');
  if (!scroller) return false;
  const bar = scroller.querySelector<HTMLElement>('.item-action-dock');
  const inBar = Boolean(control.closest('.item-action-dock'));
  const bounds = scroller.getBoundingClientRect();
  const top = bounds.top + (inBar ? 0 : (bar?.getBoundingClientRect().height ?? 0) + 12);
  if (!inBar && revealSection && section) {
    scroller.scrollTop += section.getBoundingClientRect().top - top;
  }
  if (!inBar || !bar?.classList.contains('is-stuck')) {
    const rect = control.getBoundingClientRect();
    if (rect.top < top) scroller.scrollTop += rect.top - top;
    else if (rect.bottom > bounds.bottom) scroller.scrollTop += rect.bottom - bounds.bottom;
  }
  revealMenuControl(control);
  revealShelfControl(control);
  return true;
}

function revealMenuControl(control: HTMLElement): void {
  const menu = control.closest<HTMLElement>('.item-more-menu');
  if (menu) {
    const menuBounds = menu.getBoundingClientRect();
    const rect = control.getBoundingClientRect();
    if (rect.top < menuBounds.top) menu.scrollTop += rect.top - menuBounds.top;
    else if (rect.bottom > menuBounds.bottom) menu.scrollTop += rect.bottom - menuBounds.bottom;
  }
}

function revealShelfControl(control: HTMLElement): void {
  const row = control.closest<HTMLElement>('[data-shelf-row]');
  if (row) {
    const bounds = row.getBoundingClientRect();
    const rect = control.getBoundingClientRect();
    if (rect.left < bounds.left) row.scrollLeft += rect.left - bounds.left;
    else if (rect.right > bounds.right) row.scrollLeft += rect.right - bounds.right;
  }
}

/** Dismiss secondary actions before Escape or the controller's Back can leave the page. */
function closeItemActions(): boolean {
  const menu = document.querySelector<HTMLDetailsElement>('#item-more[open]');
  if (!menu) return false;
  menu.open = false;
  menu.querySelector<HTMLElement>('summary')?.focus({ preventScroll: true });
  return true;
}

/** Close held-Resume options and return to their trigger before leaving the page. */
function closeResumeOptions(): boolean {
  const menu = document.querySelector<HTMLElement>('.resume-playback-menu:not([hidden])');
  if (!menu) return false;
  menu.hidden = true;
  const trigger = document.querySelector<HTMLElement>(`[data-resume-hold][aria-controls="${menu.id}"]`);
  trigger?.dispatchEvent(new Event('resume-options-close'));
  trigger?.focus({ preventScroll: true });
  return true;
}

function revealGuideTime(guide: HTMLElement, control: HTMLElement): void {
  if (control.closest('.tv-guide-channel')) return;
  const bounds = guide.getBoundingClientRect(), rect = control.getBoundingClientRect();
  const left = bounds.left+(guide.querySelector('.tv-guide-channel')?.getBoundingClientRect().width ?? 0);
  const intersects = rect.right > left && rect.left < bounds.right;
  // Long broadcasts may cover the whole viewport; moving down should keep the
  // current time visible rather than jumping to the far end of that program.
  if (rect.width > bounds.right-left && intersects) return;
  if (rect.left < left) guide.scrollLeft += rect.left-left;
  else if (rect.right > bounds.right) guide.scrollLeft += rect.right-bounds.right;
}

/** Reveal guide cells inside their shared scroller, allowing for sticky time/channel headings. */
function revealTvGuideControl(control: HTMLElement): boolean {
  const guide = control.closest<HTMLElement>('#tv-guide-scroll');
  if (!guide) return false;
  const main = guide.closest<HTMLElement>('.main-shell');
  const header = document.querySelector('.home-pinned-header');
  const top = (main?.getBoundingClientRect().top ?? 0) + (header?.getBoundingClientRect().height ?? 0);
  if (main && guide.getBoundingClientRect().top > main.getBoundingClientRect().bottom-136) {
    main.scrollTop += guide.getBoundingClientRect().top-top-12;
  }
  const bounds = guide.getBoundingClientRect(), rect = control.getBoundingClientRect();
  const timeHeight = guide.querySelector('.tv-guide-time-row')?.getBoundingClientRect().height ?? 48;
  const visibleTop = Math.max(bounds.top+timeHeight,top);
  const bottom = Math.min(bounds.bottom, main?.getBoundingClientRect().bottom ?? innerHeight);
  if (rect.top < visibleTop) guide.scrollTop += rect.top-visibleTop;
  else if (rect.bottom > bottom) guide.scrollTop += rect.bottom-bottom;
  revealGuideTime(guide,control);
  return true;
}

function focusControl(control: HTMLElement | undefined, revealSection = false): void {
  if (!control) return;
  control.focus({ preventScroll: true });
  const section = control.closest<HTMLElement>(SECTION_SELECTOR);
  if (revealTvGuideControl(control)) {syncNavigationState(); return;}
  if (revealItemControl(control, section, revealSection)) {
    syncNavigationState();
    return;
  }
  // Pinned controls are already visible; scrolling them would reset the media rows below.
  if (!control.closest('.home-pinned-header') || control.closest('.search-popover')) {
    if (revealSection && section?.dataset.navSection !== 'libraries') {
      section?.scrollIntoView({ block: 'start', inline: 'nearest', behavior: 'instant' });
    }
    control.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  }
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
  const selectHint = document.querySelector<HTMLElement>('[data-select-navigation-hint]');
  if (selectHint) selectHint.hidden = active?.tagName !== 'SELECT';
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

/** Left and right adjust a select without leaving its section. */
function adjustFocusedSelect(active: HTMLElement | null, direction: NavigationDirection): boolean {
  if (active?.tagName !== 'SELECT' || (direction !== 'left' && direction !== 'right')) return false;
  const select = active as HTMLSelectElement;
  const nextIndex = Math.max(0, Math.min(select.options.length - 1, select.selectedIndex + (direction === 'right' ? 1 : -1)));
  if (nextIndex !== select.selectedIndex) {
    select.selectedIndex = nextIndex;
    select.dispatchEvent(new Event('change', { bubbles: true }));
  }
  return true;
}

function moveFocus(direction: NavigationDirection): void {
  const sections = navigationSections();
  const active = document.activeElement as HTMLElement | null;
  if (adjustFocusedSelect(active, direction)) return;
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
  } else if ((direction === 'right' || (direction === 'down' && section.tagName === 'HEADER')) && section === libraries) {
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
  if (event.key === 'Escape' && (closeResumeOptions() || closeItemActions())) {
    event.preventDefault();
    return;
  }
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
  const suspended = () => state.isPlayerOpen || Boolean(state.activeTrailer) || state.route.page === 'settings' || document.hidden || Boolean(document.querySelector('dialog.book-reader[open]'));
  globalThis.addEventListener('pointermove', (event) => {
    if (event.movementX || event.movementY) setInputMode('pointer');
  }, { passive: true });
  globalThis.addEventListener('pointerdown', () => setInputMode('pointer'), { passive: true });
  globalThis.addEventListener('focusin', syncNavigationState);
  globalThis.addEventListener('keydown', (event) => {
    if (document.querySelector('dialog.book-reader[open]')) return;
    if (suspended() || event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.defaultPrevented) {
      if ((event.target as HTMLElement | null)?.matches('[data-resume-hold]') && ['Enter', ' '].includes(event.key)) setInputMode('keyboard');
      return;
    }
    handleNavigationKey(event);
  });

  const readActions = createGamepadNavigationReader();
  let resumeHold: { control: HTMLElement; gamepadIndex: number } | undefined;
  const cancelResumeHold = (): void => {
    resumeHold?.control.dispatchEvent(new Event('resume-hold-cancel'));
    resumeHold = undefined;
  };
  const updateResumeHold = (gamepads: readonly (Gamepad | null)[], actions: GamepadNavigationAction[]): void => {
    if (resumeHold) {
      const gamepad = Array.from(gamepads).find((pad) => pad?.index === resumeHold!.gamepadIndex);
      if (suspended() || !resumeHold.control.isConnected || document.activeElement !== resumeHold.control || !gamepad?.connected || actions.some((action) => action !== 'activate')) {
        cancelResumeHold();
      } else if (!gamepad.buttons[0]?.pressed) {
        const control = resumeHold.control;
        resumeHold = undefined;
        control.dispatchEvent(new Event('resume-hold-end'));
      }
    }
  };
  const activateGamepadControl = (gamepads: readonly (Gamepad | null)[]): void => {
    const focused = document.activeElement as HTMLElement | null;
    const gamepad = Array.from(gamepads).find((pad) => pad?.connected && pad.buttons[0]?.pressed);
    if (!focused?.matches('[data-resume-hold]') || !gamepad) {
      activateFocusedElement();
      return;
    }
    if (!resumeHold) {
      resumeHold = { control: focused, gamepadIndex: gamepad.index };
      focused.dispatchEvent(new Event('resume-hold-start'));
    }
  };
  const dispatchGamepadAction = (action: GamepadNavigationAction, gamepads: readonly (Gamepad | null)[]): void => {
    if (action === 'previous-section' || action === 'next-section') moveSection(action === 'next-section' ? 1 : -1);
    else if (action === 'activate') activateGamepadControl(gamepads);
    else if (action === 'back') {
      if (!closeResumeOptions() && !closeItemActions() && !returnToBrowseTabs()) globalThis.history.back();
    } else moveFocus(action);
  };
  const poll = (now: number) => {
    const gamepads = navigator.getGamepads?.() ?? [];
    const actions = readActions(gamepads, now, suspended());
    updateResumeHold(gamepads, actions);
    if (actions.length) setInputMode('gamepad');
    for (const action of actions) {
      if (suspended()) break;
      dispatchGamepadAction(action, gamepads);
    }
    globalThis.requestAnimationFrame(poll);
  };
  globalThis.requestAnimationFrame(poll);
}
