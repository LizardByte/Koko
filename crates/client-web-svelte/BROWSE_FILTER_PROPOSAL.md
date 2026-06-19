# B6 Proposal — browse-detail (collections / categories / playlists)

Status: **decision made — Option A (real SvelteKit routes, no store).** See
"Decision" at the bottom. The investigation below documents why.

## How vanilla does it

Vanilla is **route-based**, not store-based:

- `routes.ts:38-55` parses three URL shapes into a `browse-detail` route:
  - `/items/collections/:key`
  - `/items/categories/:key`
  - `/items/playlists/:key`
  - plus the library-scoped `/libraries/:id/items/(collections|categories|playlists)/:key`
- Clicking a collection/category/playlist card calls
  `navigateTo(browseDetailPath(kind, key))` (`eventBindings.ts:658,669,687,689,
  195`) — a **real URL push**.
- `homeView.ts:280-295` `renderBrowseDetailPage()` dispatches on `route.kind`
  to `renderCollectionDetailPage()` / `renderCategoryDetailPage()` /
  `renderPlaylistDetailPage()`.
- The in-memory `state.browseFilter` is a **secondary fallback**
  (`homeView.ts:112-151` `renderBrowseFilterDetail()`), rendered only when a
  filter is set but the route isn't `browse-detail`. It's cleared on every
  navigation (`eventBindings.ts:619,639,695`).

So: URLs are bookmarkable, back-button works, deep links work, and the store
is just a transient overlay.

## Option A — Real routes (match vanilla)

Add `src/routes/items/[kind]/[key]/+page.svelte` (or three concrete dirs)
handling `collections|categories|playlists`, plus the library-scoped
`libraries/[id]/items/[kind]/[key]`. Port `renderBrowseDetailPage` +
`renderCollectionDetailPage` + `renderCategoryDetailPage` +
`renderPlaylistDetailPage` into a `BrowseDetail.svelte` component. Replace
the three dead `goto`s with `goto(browseDetailPath(kind, key))`.

- **UX:** identical to vanilla — bookmarkable URLs, browser back/forward,
  deep-linkable, reload-safe.
- **Implementation:** SvelteKit file-based routing makes this natural. A
  `BrowseDetail` component + a `browseDetailPath()` helper (port of
  `homeView.ts:41-52`) + the three render functions. The category/playlist
  data comes from existing selectors (`categorySummaries`,
  `collectionSummaries`) already in the port. Medium effort (~1 component,
  ~3 route files, ~150 lines).
- **Cost:** more files; need to handle the library-scoped path variant.

## Option B — Store action only (no routes)

Add `catalog.browseFilter` state + `setBrowseFilter({kind, key, …})`. Render
browse-detail inline in `HomeContent` when a filter is active (port
`renderBrowseFilterDetail`). Replace the three `goto`s with
`catalog.setBrowseFilter(…)`.

- **UX:** **worse than vanilla.** No URL, no back button, no deep link, no
  reload survival. Refresh loses the view. The browser back button exits the
  app page instead of clearing the filter.
- **Implementation:** less file churn (one store field, one inline render
  branch). Lower effort (~100 lines).
- **Cost:** a real UX regression vs vanilla, which violates the fidelity
  contract. Vanilla *had* this as a fallback but primary navigation was
  route-based.

## Option C — Hybrid (route + store, exactly like vanilla)

Real routes (Option A) **plus** a `browseFilter` store overlay that
`renderBrowseFilterDetail` uses when set-but-not-on-a-route. This is a
faithful 1:1 port of vanilla's two mechanisms.

- **UX:** matches vanilla exactly, including the edge case where a filter is
  set without a navigation (rare; mostly the clear-filter flow).
- **Implementation:** A + the store field + the fallback render branch.
  Highest effort (~200 lines).
- **Cost:** most code; the fallback branch is rarely exercised.

## Recommendation: Option A

Vanilla's primary mechanism is routes; the store fallback exists but is
transient and rarely visible. Option A reproduces the user-facing behaviour
(bookmarkable, back-button, deep links) with the least code that does so, and
avoids introducing a store field whose semantics (set vs route) can drift.
If we later find a real need for the fallback (e.g. a flow that sets a filter
without navigating), upgrading A → C is additive.

**Concrete plan if A is approved:**
1. `src/lib/components/BrowseDetail.svelte` — ports
   `renderBrowseDetailPage` + the three sub-renderers (collection hero +
   item grid, category hero + grid, playlist placeholder).
2. `src/lib/paths.ts` (or extend `selectors.ts`) — `browseDetailPath(kind,
   key, libraryId?)` matching `homeView.ts:41-52`.
3. `src/routes/items/[kind]/[key]/+page.svelte` — validates `kind ∈
   {collections,categories,playlists}`, renders `<BrowseDetail>`.
4. `src/routes/libraries/[id]/items/[kind]/[key]/+page.svelte` — same,
   library-scoped.
5. Replace the three dead `goto('/collections/...')` callsites with
   `goto(browseDetailPath('collection', id))`.
6. Wire the Categories/Playlists tab cards (currently empty-states) to
   navigate via `browseDetailPath('category'|'playlist', …)`.

---

## Investigation: why does vanilla have the `browseFilter` store field?

**Answer: it doesn't, in practice. The store branch is dead code.**

A full grep of `crates/client-web/src/` for `state.browseFilter` assignments
shows it is **only ever set to `undefined`** (5 sites: eventBindings.ts:619,
639, 695 and app.ts:273, 856). It is **never assigned a real value**. So:

- `renderBrowseFilterDetail()` (homeView.ts:113)
  `state.route.page === 'browse-detail' ? browseFilterForRoute() : state.browseFilter`
  always takes the `browseFilterForRoute()` branch (route-driven) on a
  browse-detail route, and always returns the empty-state otherwise (because
  `state.browseFilter` is always `undefined`).
- The `state.browseFilter` reads at homeView.ts:797,804,825,917 and
  selectors.ts:235 are therefore unreachable.

Conclusion: the "hybrid" in Option C is illusory. Vanilla's store field is a
vestigial fallback (likely from an earlier in-memory-filtering design) that
was never wired up. **Option A (real routes) is a complete, faithful port —
there is nothing the store adds.**

## Can a SvelteKit feature replace the manual `browseFilter`?

**Yes — and we need no store state at all.** `browseFilterForRoute()`
(homeView.ts:69-110) is a **pure derivation** from two inputs:

1. The route (`route.kind` + `route.key`, optionally `libraryId`) — these map
   directly to SvelteKit's `page.params`, read in a `+page.ts` `load` or
   `$app/state`'s `page` in the component.
2. Existing catalog data:
   - `collectionSummaries()` → in the port this is `catalog.home?.collections`
     (already in the `catalog` store; vanilla selectors.ts:165-167 is just
     `state.home?.collections ?? []`).
   - `categorySummaries()` → **not yet ported** (the Categories tab is a stub).
     Vanilla derives it from `state.libraryItems` grouped by genre
     (selectors.ts:135-163). This is the one piece of new selector work.

So the idiomatic SvelteKit implementation is:

- **No `browseFilter` store field.** Delete `BrowseFilter` state entirely from
  the catalog store plan; it was never going to hold anything.
- A `BrowseDetail.svelte` component that reads `kind`/`key`/`libraryId` from
  `page.params` and derives its data from `catalog` (collections are already
  there; categories via a new `categorySummaries()` selector).
- Route files: `src/routes/items/[kind]/[key]/+page.svelte` and the
  library-scoped `src/routes/libraries/[id]/items/[kind]/[key]/+page.svelte`.
  A `+page.ts` `load` can validate `kind ∈ {collections,categories,playlists}`
  and 404 otherwise.
- `browseDetailPath()` helper for the card `goto()` calls.

## Decision

**Option A — real SvelteKit routes, no store.**

Rationale:
- Vanilla is route-driven in practice (the store is dead code, verified).
- Route-driven gives bookmarkable URLs, back/forward, deep links — the UX
  vanilla actually ships.
- SvelteKit's `page.params` + `load` make the route inputs idiomatic; no
  manual filter state to keep in sync.
- Zero new store fields; the only new logic is a `categorySummaries` selector
  (needed for the Categories tab regardless).

Scheduled for Phase 6. Prerequisite: port `categorySummaries()` (also unblocks
the Categories home tab, currently a stub).


Falls within Phase 6 (cross-cutting). No playback, no settings dependency.
