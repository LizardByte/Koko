import type { AppRoute, HomeBrowseTab } from './types';
import { state } from './state';

interface BrowseVisit {
  tab: HomeBrowseTab;
  page: number;
  filtersOpen: boolean;
  search: string;
  fullSearch: boolean;
  scrollTop: number;
  windowY: number;
  focusItem?: string;
  focusClass?: string;
  shelves: Array<[string, number]>;
  data?: Pick<typeof state, 'home' | 'libraryItems' | 'searchResults' | 'homePreviewItemId' | 'homePreviewCollectionId' | 'browsePreferences'>;
}

const visits = new Map<string, BrowseVisit>();
const returnPaths = new Map<number | undefined, string>();
let pendingRestore: string | undefined;

export function clearBrowseSessions(): void {
  visits.clear();
  returnPaths.clear();
  pendingRestore = undefined;
}

export function browsePath(route: AppRoute): string | undefined {
  if (route.page !== 'home' && route.page !== 'browse-detail') return undefined;
  const prefix = route.libraryId === undefined ? '' : `/libraries/${route.libraryId}`;
  if (route.page === 'home') return prefix || '/';
  const segment = route.kind === 'category' ? 'categories' : `${route.kind}s`;
  return `${prefix}/items/${segment}/${encodeURIComponent(route.key)}`;
}

function visitKey(route: AppRoute): string | undefined {
  const path = browsePath(route);
  return path ? `${state.bootstrap?.current_user?.id ?? 'guest'}:${path}` : undefined;
}

export function captureBrowseSession(): void {
  const key = visitKey(state.route);
  if (!key) return;
  const path = browsePath(state.route)!;
  const focusItem = (document.activeElement as HTMLElement | null)?.dataset.itemId;
  const active = document.activeElement;
  const focusClass = ['.media-card', '.search-result-row'].find((selector) => active?.matches(selector)) ?? '.home-feature-action';
  visits.set(key, {
    tab: state.homeTab, page: state.libraryPage, filtersOpen: state.browseFiltersOpen, search: state.searchQuery, fullSearch: state.showFullSearchResults,
    scrollTop: document.querySelector('.main-shell')?.scrollTop ?? 0, windowY: globalThis.scrollY,
    focusItem, focusClass, shelves: Array.from(document.querySelectorAll<HTMLElement>('[data-shelf-row]')).map((row) => [row.dataset.shelfRow!, row.scrollLeft]),
    data: state.home && !state.libraryItemsLoading ? {
      home: state.home, libraryItems: state.libraryItems, searchResults: state.searchResults,
      homePreviewItemId: state.homePreviewItemId, homePreviewCollectionId: state.homePreviewCollectionId,
      browsePreferences: { ...state.browsePreferences },
    } : visits.get(key)?.data,
  });
  returnPaths.set(state.bootstrap?.current_user?.id, path);
  if (visits.size > 30) visits.delete(visits.keys().next().value!);
}

export function browseReturnPath(): string | undefined {
  return returnPaths.get(state.bootstrap?.current_user?.id);
}

export function restoreBrowseSession(): boolean {
  const key = visitKey(state.route);
  const visit = key ? visits.get(key) : undefined;
  pendingRestore = visit ? key : undefined;
  state.libraryPage = visit?.page ?? 0;
  state.browseFiltersOpen = visit?.filtersOpen ?? false;
  if (!visit) return false;
  state.homeTab = visit.tab;
  state.searchQuery = visit.search;
  state.showFullSearchResults = visit.fullSearch;
  if (visit.data) {
    Object.assign(state, visit.data);
    state.libraryItemsLoading = false;
  }
  return true;
}

/** Restore once data is ready, after DOM patching has restored the previous page's snapshot. */
export function restoreBrowsePosition(): void {
  if (!pendingRestore || state.libraryItemsLoading) return;
  const key = pendingRestore;
  const visit = visits.get(key);
  if (!visit || key !== visitKey(state.route)) return;
  pendingRestore = undefined;
  requestAnimationFrame(() => {
    if (key !== visitKey(state.route)) return;
    const main = document.querySelector('.main-shell');
    if (main) main.scrollTop = visit.scrollTop;
    globalThis.scrollTo({ top: visit.windowY });
    for (const [id, left] of visit.shelves) {
      const row = document.querySelector<HTMLElement>(`[data-shelf-row="${CSS.escape(id)}"]`);
      if (row) row.scrollLeft = left;
    }
    if (visit.focusItem && ['keyboard', 'gamepad'].includes(document.body.dataset.inputMode ?? '')) {
      document.querySelector<HTMLElement>(`.main-shell ${visit.focusClass}[data-item-id="${CSS.escape(visit.focusItem)}"]`)?.focus({ preventScroll: true });
    }
  });
}
