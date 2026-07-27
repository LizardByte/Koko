# Phase 7 — Verification & Fidelity Report

**Date:** 2026-06-20
**Status:** Partial verification (pre-player). The player spike is the final feature gap.

---

## Port Completeness

### Inventory
| Category | Count | Detail |
|---|---|---|
| Components | 42 | 33 root + 9 settings sub-components |
| Routes | 15 | 2 layouts + 13 pages |
| Stores | 7 | auth, catalog, item, libraries, settings, activities, ui |
| Stories | 96 | 30 story files across 30 groups |
| Mock endpoints | 24 | Every API method has a mock handler |
| svelte-check | 0 errors | 11 pre-existing warnings |
| Production build | ✅ | adapter-static, SPA mode |

### Routes — all vanilla routes ported
- `/` — Home (all libraries)
- `/libraries/[id]` — Library home
- `/items/[id]` — Item detail (hero, people, extras, children, support, metadata-link)
- `/items/[kind]/[key]` — Browse listing (collection/category/playlist)
- `/libraries/[id]/items/[kind]/[key]` — Per-library browse listing
- `/people/[id]` — Person detail (hero, credits)
- `/login` — Login screen
- `/settings` — General + Users
- `/settings/libraries` — Library management
- `/settings/providers` — Metadata provider config
- `/settings/scheduled` — Scheduled tasks
- `/settings/dashboard` — Metadata dashboard + system activities
- `/settings/logs` — Log viewer

### The single missing feature
**The player.** Vanilla `playbackController.ts` (1385 lines) + `youtube.ts` (115 lines) +
`mediaTargets.ts` (63 lines) + `mediaExtras.ts` (33 lines) = ~1600 lines of playback logic.
The Svelte port has stubs only (HeroActions/SectionExtras surface "not yet implemented" errors).

What the vanilla player does:
- HTML5 `<video>`/`<audio>` overlay with session-based streaming (createPlaybackSession)
- Dual audio track support with server-side remuxing
- Subtitle track injection
- Picture-in-Picture
- Fullscreen
- Escalating seek (10→20→30→60→120→300s steps)
- Progress reporting (POST position/duration/completed)
- YouTube IFrame trailer overlay + ambient theme-song player
- Transcoding badge when audio stream is overridden

---

## Improvements Upon Vanilla (Deliberate Deltas)

These are enhancements the Svelte port makes beyond vanilla, documented for fidelity review.

### Architecture
1. **Component taxonomy** — vanilla is ~15 flat render functions in `app/`. The port uses a consistent naming system: `Section*` (page-stack members), `BrowseListing` (routed page), unprefixed (leaf/sub-component). Scales as the app grows.
2. **CardSurface primitive** — vanilla duplicates the transparent-button + rounded-tile pattern across 3 cards. The port extracts `CardSurface.svelte` so card bugs (overflow, hover-leak) are fixed in one place.
3. **Reactive polling** — vanilla uses `setTimeout` chains + snapshot-diff JSON comparison + `maybeRenderAfterAutoRefresh` to avoid unnecessary re-renders. The port uses Svelte `$effect` with cleanup — fine-grained reactivity eliminates the snapshot-diff + force-render machinery entirely.

### CSS / Visual fixes
4. **MediaCard corner bleed fix** — vanilla's `.media-card-art` lacks `overflow: hidden`, so gradient fallbacks and oversized images bleed past the 18px radius. The port adds `overflow: hidden` (documented delta in POSTMORTEM.md).
5. **MediaCard hover-leak fix** — vanilla's global `button:hover` lifts + glows the entire card (including the text block below the poster). The port suppresses the global hover on `.media-card` and lifts only the tile with a blue shadow — the intended behavior.
6. **Button spinner centering** — vanilla's `button.is-busy::after` has `position: absolute` but no centering, so the spinner renders off-center to the right. The port adds `inset: 0; margin: auto` (scoped to Button.svelte).
7. **Rail keyboard accessibility** — vanilla's person-credit cards have `tabindex="0"` on `<article>` elements (a11y warning). The port adds `role="button"` + `aria-label` + Enter/Space keydown handlers, making them genuine keyboard-activatable controls.

### Settings enhancements
8. **Dashboard table sorting** — vanilla sorts by refresh-state rank only. The port adds clickable column headers (Title/Type/Library/Refresh state/Artwork updated) with asc/desc toggle — a real UX improvement for browsing 100s of items.
9. **Provider priority reordering** — vanilla uses DOM `insertBefore` manipulation + re-sync. The port reorders the `settings.metadata.providers` array reactively — cleaner, no DOM mutation.
10. **Profile-image preview** — vanilla shows no preview before upload. The port shows an object-URL preview that's revoked after save.

### Developer experience (Storybook)
11. **96 stories across 30 groups** — vanilla has no Storybook. The port has full component isolation with: preset dropdown (no typos), dark docs theme (via Storybook theme API), CC0 artworks (real images for fixture items), Icon color/alpha controls, store-driven disclaimers, per-category organization (Components/Fragments/Screens).
12. **Preset dropdown** — the `preset` arg is a select dropdown (9 options) instead of free text, preventing typos and making available fixture bundles visible.
13. **Mock API** — 24 endpoints fully mocked with realistic fixtures, so the port runs without a backend in Storybook + dev mode.

---

## Known Gaps (beyond the player)

These are minor items noted during development, not blocking:

1. **Playback** — all play/trailer/theme handlers are stubs (the player spike).
2. **Library never-scanned polling** — vanilla has a separate 1800ms timer for libraries with `status === 'never_scanned'`. The port's poll covers metadata-refresh + library-scan activities but not the never-scanned initial-scan case. Low priority (mock doesn't seed never-scanned libraries).
3. **Pre-existing story gaps** — LoginScreen, WelcomeScreen, AuthShell are covered indirectly via the Screens/Auth stories but have no standalone story files. IconPreview is a story-only wrapper (not a real component).

---

## Fidelity Verification Method

Every component was verified by:
1. **Source diff** against vanilla — CSS values, DOM structure, event handlers compared line-by-line
2. **svelte-check** — 0 TypeScript errors throughout
3. **Playwright smoke** — 96/96 stories render without errors (headless browser, canvas mode)
4. **Production build** — adapter-static SPA build succeeds
5. **Mock API parity** — every `api.ts` method has a mock handler with matching fixtures
