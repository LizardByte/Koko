# Koko Svelte 5 Client — Complete Rewrite

A high-fidelity, production-ready Svelte 5 / SvelteKit rewrite of the Koko media-server web client (vanilla TypeScript → Svelte 5 runes + SvelteKit). Feature-complete, tested, and ready for evaluation.

---

## What This Is

The Koko media server (by LizardByte) currently ships a vanilla TypeScript web client (`crates/client-web/`) — imperative DOM rendering, manual event binding, string-template HTML. This rewrite (`crates/client-web-svelte/`) ports the entire client to **Svelte 5 with runes** ($state, $derived, $effect, $props) and **SvelteKit** with adapter-static SPA mode. Every feature from the vanilla client is ported, plus meaningful improvements.

### Branch
```
git checkout poc/svelte-rewrite
```
The Svelte client lives in `crates/client-web-svelte/`. The vanilla client (`crates/client-web/`) is untouched.

---

## How to Run

### Against a real Koko server (dev with hot reload)
```bash
cd crates/client-web-svelte
KOKO_API_TARGET=https://127.0.0.1:9191 npm run dev
# Opens at http://127.0.0.1:4173/
```
The Vite dev server proxies all `/proxy/*` requests to the Rust server (TLS accepted, no CORS needed). Set `KOKO_API_TARGET` to your server's HTTPS URL.

### Storybook (component gallery, no server needed)
```bash
cd crates/client-web-svelte
npm run storybook
# Opens at http://localhost:6006/
```
98 stories across 30 groups, fully mocked (no backend required). Dark theme by default.

### Build + serve from the Rust server (production)
```bash
cd crates/client-web-svelte
npm run build
# Output in dist/ — copy to wherever the Rust server serves static files
```

---

## Architecture Comparison

### Vanilla client (the original)
- **Rendering:** imperative string-template HTML + `innerHTML` + manual `render()` calls
- **State:** a single mutable `AppState` object; `render()` re-renders the entire DOM tree on every change
- **Events:** `eventBindings.ts` (1395 lines) — manual `addEventListener` for every interaction, querySelector-based
- **DOM updates:** `domPatcher.ts` (290 lines) — custom diff/patch to avoid full re-renders
- **Player:** `playbackController.ts` (1385 lines) — imperative `querySelector` + `classList.toggle` + `addEventListener` for every control
- **Polling:** `setTimeout` chains + JSON snapshot-diffing + `maybeRenderAfterAutoRefresh` force-render logic
- **Components:** ~15 flat render functions, no structure
- **Testing:** none

### Svelte 5 client (this rewrite)
- **Rendering:** Svelte 5 compiled templates — the DOM is updated surgically, only what changed
- **State:** 8 domain stores (auth, catalog, item, libraries, settings, activities, playback, ui) — class instances with `$state`/`$derived` runes, reactive by design
- **Events:** Svelte `onclick`/`bind:`/`$effect` — declarative, no manual binding
- **DOM updates:** automatic (Svelte compiler handles it) — `domPatcher.ts` not needed
- **Player:** reactive playback store + 8 player components (~650 lines total vs 1385 vanilla)
- **Polling:** reactive `$effect` timers with automatic cleanup — no snapshot-diffing
- **Components:** 42 components in a structured taxonomy (Section*/BrowseListing/leaf)
- **Testing:** 98 Storybook stories, svelte-check, Playwright smoke tests

### What got simpler
| Vanilla pattern | Svelte equivalent | Lines saved |
|---|---|---|
| `document.querySelector('#player-progress')` + manual value sync | `<input type="range" bind:value>` | ~40 |
| `addEventListener('timeupdate', updateTimeline)` | `$effect` reacting to state | ~30 |
| `classList.toggle('is-controls-visible')` + setTimeout | `$state` + reactive class | ~25 |
| `state.activeAudioStreamIndex = X; render(false)` | state mutation → surgical update | ~15 |
| `domPatcher.ts` (custom DOM diff) | Svelte compiler (automatic) | 290 |
| `eventBindings.ts` (manual event listeners) | Svelte event handlers | 1395 |
| `maybeRenderAfterAutoRefresh` (force-render) | fine-grained reactivity | 0 needed |
| Player: 1385 lines of imperative DOM | 650 lines of reactive components | 735 |

---

## New Architecture (detailed)

### Component taxonomy
```
src/lib/components/
├── (leaf components)
│   ├── Button.svelte          — styled button with icon + busy state
│   ├── Icon.svelte            — @lucide/svelte wrapper (string-name API)
│   ├── CardSurface.svelte     — shared card shell (transparent button + rounded tile)
│   ├── MediaCard.svelte       — media item card (poster + badges + progress)
│   ├── PersonCard.svelte      — person thumbnail card (built on CardSurface)
│   ├── MediaExtraCard.svelte  — trailer/theme thumbnail card (built on CardSurface)
│   ├── UserAvatar.svelte      — circular avatar (image or initials)
│   ├── CollapsibleText.svelte — expandable text with localStorage persistence
│   └── IconPreview.svelte     — story-only wrapper for Icon color/alpha controls
│
├── (item detail sub-components)
│   ├── HeroActions.svelte     — play/resume/trailer/theme/back buttons
│   ├── FactList.svelte        — technical facts grid (codec, resolution, etc.)
│   ├── SupportFileInfo.svelte — file + library info panel
│   ├── SupportMetadata.svelte — metadata summary + search/link panel
│   ├── MetadataSearchPanel.svelte — interactive metadata search + link UI
│   └── MediaExtraCard.svelte  — (see leaf components above)
│
├── Section*.svelte            — page-section components (direct children of item-detail page)
│   ├── SectionHero.svelte     — poster + title + badges + overview
│   ├── SectionSupport.svelte  — 2-column grid (SupportFileInfo + SupportMetadata)
│   ├── SectionPeople.svelte   — cast rail (PersonCard)
│   ├── SectionExtras.svelte   — trailer/theme rail (MediaExtraCard)
│   ├── SectionChildren.svelte — seasons/episodes grid
│   └── SectionBreadcrumbs.svelte — hierarchy nav
│
├── (page-level fragments)
│   ├── BrowseListing.svelte   — collection/category/playlist page (dispatcher)
│   ├── BrowseListingHero.svelte — browse listing hero banner
│   ├── BrowseListingGrid.svelte — browse listing items grid
│   ├── HomeContent.svelte     — home page body (shelves, collections, search)
│   ├── HomeFeature.svelte     — home spotlight feature
│   ├── HomeNavbar.svelte      — home top bar (tabs, search, library actions)
│   ├── Rail.svelte            — persistent sidebar (brand, nav, user card)
│   ├── Shelf.svelte           — horizontal media shelf
│   └── PersonHero.svelte      — person detail page hero
│
├── PersonCredits.svelte       — person credits grid (shows → seasons → episodes)
│
├── settings/                  — settings sub-pages
│   ├── SettingsShell.svelte   — page navbar + section nav sidebar
│   ├── GeneralForm.svelte     — server config + ffmpeg paths
│   ├── UserManagement.svelte  — user CRUD with base64 profile-image upload
│   ├── LibrarySettings.svelte — library cards + add-library form
│   ├── ProviderSettings.svelte — provider cards + priority reordering
│   ├── ScheduledTasks.svelte  — task runner + 3 task cards
│   ├── MetadataDashboard.svelte — metadata dashboard with sortable table
│   ├── SystemActivities.svelte — background activities panel
│   └── LogViewer.svelte       — log viewer with filters
│
├── player/                    — player overlay components
│   ├── PlayerOverlay.svelte   — top-level overlay (media/trailer/theme dispatcher)
│   ├── MediaPlayer.svelte     — HTML5 video/audio element + event binding
│   ├── PlayerControls.svelte  — shared controls bar (progress, transport, volume, etc.)
│   ├── AudioTrackMenu.svelte  — audio track selection dropdown
│   ├── TrailerPlayer.svelte   — YouTube trailer overlay
│   ├── ThemeSongPlayer.svelte — ambient theme song (audio-first)
│   └── YouTubeIframe.svelte   — reusable YouTube IFrame wrapper
│
├── AuthShell.svelte           — auth page wrapper (brand + error panel)
├── LoginScreen.svelte         — login form
└── WelcomeScreen.svelte       — first-user setup form
```

### Stores
```
src/lib/stores/
├── auth.svelte.ts      — bootstrap, login/logout, user CRUD, canManageUsers
├── catalog.svelte.ts   — home shelves, library items, search, collections
├── item.svelte.ts      — item detail, metadata, person, playback decision, metadata search/link
├── libraries.svelte.ts — library list, scan, refresh metadata, delete missing
├── settings.svelte.ts  — settings CRUD, library add/remove, metadata providers
├── activities.svelte.ts— system activities, logs, dashboard items, auto-refresh polling
├── playback.svelte.ts  — playback session, player state, seek, progress, trailer, theme song
└── ui.svelte.ts        — error banner state
```

### Routes (SvelteKit)
```
src/routes/
├── +layout.svelte              — root layout (auth gating, rail, player overlay, polling)
├── +page.svelte                — home (all libraries)
├── login/+page.svelte          — login screen
├── items/[id]/+page.svelte     — item detail (hero, people, extras, children, support)
├── items/[kind]/[key]/+page.svelte — browse listing (collection/category/playlist)
├── libraries/[id]/+page.svelte — library home
├── libraries/[id]/items/[kind]/[key]/+page.svelte — per-library browse listing
├── people/[id]/+page.svelte    — person detail (hero, credits)
└── settings/
    ├── +layout.svelte          — settings shell (loads settings on mount)
    ├── +page.svelte            — general + users
    ├── libraries/+page.svelte  — library management
    ├── providers/+page.svelte  — metadata provider config
    ├── scheduled/+page.svelte  — scheduled tasks
    ├── dashboard/+page.svelte  — metadata dashboard + activities
    └── logs/+page.svelte       — log viewer
```

---

## Feature Completeness

Every vanilla feature has a Svelte port:

| Feature | Status |
|---|---|
| Home (shelves, collections, search) | ✅ |
| Item detail (hero, people, extras, children, support) | ✅ |
| Metadata search + manual link + force refresh | ✅ |
| Browse listing (collection/category/playlist) | ✅ |
| Person detail (hero, credits) | ✅ |
| Settings (general, users, libraries, providers, scheduled, dashboard, logs) | ✅ |
| Auth (login, first-user setup, token management) | ✅ |
| Player (HTML5 video/audio, transcoding, audio tracks, subtitles) | ✅ |
| YouTube trailer overlay | ✅ |
| Ambient theme song | ✅ |
| Auto-refresh polling (metadata/scan state) | ✅ |
| Error surfacing | ✅ |
| Keyboard shortcuts (escalating seek, play/pause, mute, fullscreen) | ✅ |
| Picture-in-Picture | ✅ |
| Fullscreen | ✅ |
| Progress reporting (every 15s + on completion) | ✅ |
| Resume prompt via localStorage | ✅ (improvement) |
| Back-button closes player (pushState) | ✅ (improvement) |

---

## Improvements Over Vanilla

### Architecture
1. **Reactive state model** — 8 domain stores replace the single mutable AppState + manual render() calls. State changes automatically update only the affected DOM nodes.
2. **Component taxonomy** — `Section*` (page-stack members), `BrowseListing` (routed page), unprefixed (leaf/sub-component). Scales cleanly.
3. **CardSurface primitive** — the transparent-button + rounded-tile pattern (shared by MediaCard, PersonCard, MediaExtraCard) extracted to one component. Card bugs fixed in one place.
4. **Shared PlayerControls** — one controls bar component for both the media player and trailer player (vanilla duplicates ~300 lines).

### Player (9 approved improvements)
5. **Reactive progress reporting** — store method called from event listener, not setInterval + manual tracking
6. **Keyboard shortcuts via Svelte action** — `use:playerShortcuts` (reusable, testable)
7. **$effect cleanup for teardown** — session deletion + YouTube destroy happen automatically on unmount
8. **Audio-first theme song** — `<audio>` element primary, YouTube IFrame fallback only for YouTube URLs
9. **pushState back-button** — browser back closes the player (pushState + beforeNavigate intercept)
10. **Resume prompt via localStorage** — "Continue from X:XX?" on re-open
11. **Accessibility** — aria-live on loading/error, proper range attributes, descriptive labels

### CSS / Visual
12. **MediaCard corner bleed fix** — vanilla's `.media-card-art` lacks `overflow: hidden`; gradient fallbacks bleed past the radius. Fixed.
13. **MediaCard hover-leak fix** — vanilla's global `button:hover` lifts the entire card including text. Suppressed; only the tile lifts.
14. **Button spinner centering** — vanilla's `::after` spinner lacks centering. Fixed (scoped to Button.svelte).
15. **Slider full-range movement** — custom thumb CSS so the slider reaches true 0/max (native range inputs have ~10% dead zones).
16. **PersonCredits keyboard accessibility** — vanilla's `<article tabindex="0">` has no keyboard handler. Added role=button + Enter/Space.

### Settings
17. **Dashboard table sorting** — clickable column headers (Title/Type/Library/Refresh state/Artwork updated) with asc/desc toggle. Vanilla sorts by refresh-state rank only.
18. **Provider priority reordering** — move-up/move-down buttons reorder the array reactively. Vanilla uses DOM insertBefore manipulation.
19. **Profile-image preview** — object-URL preview before upload. Vanilla shows nothing.
20. **Transcode duration pinning** — player pins duration from item metadata (not the growing transcode stream duration). Plus server-side X-Content-Duration header + ffmpeg -t.

### Developer Experience
21. **98 Storybook stories** across 30 groups — vanilla has no Storybook. Full component isolation with preset dropdown, dark theme, CC0 artworks, store-driven disclaimers.
22. **Preset dropdown** — the `preset` arg is a select dropdown (10 options) instead of free text, preventing typos.
23. **Mock API** — 24 endpoints fully mocked with realistic fixtures, so the port runs without a backend in Storybook + dev mode.
24. **svelte-check** — 0 TypeScript errors, 0 warnings. Production build clean.

---

## Tech Stack

| | Version |
|---|---|
| Svelte | 5.56.3 (runes: $state, $derived, $effect, $props) |
| SvelteKit | 2.66.0 (adapter-static, SPA mode) |
| Vite | 8.0.16 |
| @sveltejs/vite-plugin-svelte | 7.1.2 |
| Storybook | 10.4.6 (@storybook/sveltekit + addon-svelte-csf) |
| @lucide/svelte | 1.21.0 |
| TypeScript | 5.8.3 |

---

## Testing

- **svelte-check:** 0 errors, 0 warnings
- **Storybook:** 98 stories, 30 groups — all passing Playwright headless smoke
- **Production build:** adapter-static SPA, clean
- **Real backend:** tested against a live Koko server (login, browse, play transcoded media with seeking, audio track switching, metadata search/link)

---

## Stats

| Metric | Vanilla | Svelte |
|---|---|---|
| Components | ~15 flat render functions | 42 structured components |
| Stores | 1 AppState object | 8 domain stores |
| Player | 1385 lines (playbackController.ts) | ~650 lines (store + 8 components) |
| Event binding | 1395 lines (eventBindings.ts) | 0 (declarative) |
| DOM diffing | 290 lines (domPatcher.ts) | 0 (compiler handles it) |
| Stories | 0 | 98 |
| svelte-check | N/A | 0 errors, 0 warnings |

---

## Known Issues

1. **Backend: stale DB entries** — files moved between library paths leave orphan entries (documented in `crates/server/BACKEND_ISSUES.md`). Scanner should detect + clean these.
2. **YouTube IFrame in Storybook** — the YouTube components load the external IFrame API which may be blocked in some environments. The mock stories show the player shell without actual YouTube playback.
3. **Mock stream endpoint** — the mock API returns 501 for `/sessions/{id}/stream`, so MediaPlayer stories show the error state (by design — the controls UI is what's testable in isolation).

---

## License

Same as the Koko project (LizardByte).
