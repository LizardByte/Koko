# koko-client-web-svelte (PoC)

Proof-of-concept Svelte 5 port of the Koko web client. See
[`../../PROPOSAL.md`](../../PROPOSAL.md) for the full migration analysis.

This is **not** a complete client. It ports a representative slice of views to
de-risk the architectural decisions in the proposal:

| Route | Status | Vanilla source |
|---|---|---|
| `/` (Home) | ✅ Ported | `app/homeView.ts` `renderHomePage` |
| `/items/[id]` | ✅ Ported | `app/itemPersonView.ts` `renderItemPage` |
| `/login` | ✅ Ported | `app/auth.ts` `renderLoginScreen` |
| `/settings` | Stub | `app/settingsView.ts` (General form) |
| `/settings/libraries` | ✅ Ported (list) | `app/settingsView.ts` library list |
| `/settings/dashboard` | Stub | `app/dashboardView.ts` `renderMetadataDashboard` |
| `/settings/logs` | ✅ Ported | `app/dashboardView.ts` `renderLogViewer` |

**Deliberately out of scope** (see PROPOSAL.md §4): playback
(`playbackController.ts`), YouTube trailers (`youtube.ts`), home shelf
virtualization, metadata search/linking, settings forms, and `app.ts`'s
auto-refresh polling.

## Run it

```bash
npm install
npm run dev:mock    # http://127.0.0.1:4173/  (mock data, no backend needed)
```

Mock login: `admin` / `adminpass`.

For a real backend, drop the `:mock` and set `VITE_API_BASE_URL` to the server.

## Storybook

Component-level documentation and isolated dev environment.

```bash
npm run storybook      # http://localhost:6006
npm run build-storybook  # static build → storybook-static/
```

Stories live next to their components as `*.stories.svelte` (Svelte CSF —
`defineMeta` + `<Story>`, Svelte 5 runes-native). Every ported component has
coverage; highlights:

- **`Components/MediaCard`** — every badge variant (unmatched / watched /
  in-progress / missing / metadata-pending). The `Unmatched` story is a
  regression cage for the `has_metadata !== true` predicate.
- **`Components/Icon`** — gallery of the entire `ICONS` map (canonical set +
  missing-icon catcher).

**How stories handle global state.** Most components read the store
singletons (`catalog`, `item`, `auth`, …) and `$app/state`. Storybook isn't a
router, so:

- `.storybook/main.ts` `viteFinal` aliases `$app/state` →
  `src/lib/storybook/mockAppState.svelte.ts` (a mutable runes-based `page`)
  and `$app/navigation` → `mockAppNavigation.ts` (no-op `goto`).
- `.storybook/preview.ts` applies a global `WithStores` decorator that seeds
  the stores from a named preset before each story and resets on cleanup.
  Pick a preset per story via `args.preset`:

  | preset | what it seeds |
  |---|---|
  | `empty` | clean baseline (default) |
  | `home` | libraries + home shelves + library items + logged-in user |
  | `item-movie` / `item-show` / `item-missing` / `item-watched` | item store variant + libraries |
  | `auth-logged-in` / `requires-login` / `requires-setup` | auth bootstrap states |

  Override the mock route via `args.route` (e.g. `'/libraries/2'`) to test
  active-state rendering.

Adding fixtures: extend `src/lib/storybook/fixtures.ts` (self-contained data
builders, kept separate from `mockApi.ts`). Add a new preset in `presets.ts`.

## What this demonstrates

- **SvelteKit + `adapter-static`** produces a static `dist/` the Rust server can
  serve (SPA fallback, SSR disabled — one line in `src/routes/+layout.ts`).
- **Routing parity** with `routes.ts` via file-based routes (`/items/[id]`,
  `/settings/logs`, etc.).
- **The data layer ports verbatim** — `src/lib/api.ts` mirrors
  `../client-web/src/api.ts`'s types and the `VITE_USE_MOCK_API` toggle.
- **The `domPatcher.ts` / `eventBindings.ts` glue disappears** — compare the
  logs view here (~150 lines) to its vanilla equivalent (template string +
  three event handlers + partial re-render plumbing).
- **Reusable components** (`MediaCard`, `Shelf`, `Tag`, `Spinner`, `Icon`)
  replace the vanilla client's repeated string-template helpers.

## Layout

```
src/
  app.css                       shared styles (carries log-message-col fix)
  app.html                      SvelteKit shell
  lib/
    api.ts                      types + fetch layer + mock (mirrors vanilla api.ts/mockApi.ts)
    auth.svelte.ts              auth store (Svelte 5 class runes)
    format.ts                   pure formatters (from app/format.ts)
    activities.ts               filter-request helper (from app/activities.ts)
    components/
      Icon.svelte               inline SVG icon set (replaces lucide hydration)
      MediaCard.svelte          item poster card (replaces renderItemCard)
      Shelf.svelte              horizontal scroll rail (replaces renderRail)
      Tag.svelte, Spinner.svelte
  routes/
    +layout.svelte              app shell + auth guard (replaces startApp + ui navbar)
    +layout.ts                  SPA mode (ssr=false)
    +page.svelte                Home
    login/+page.svelte          Login
    items/[id]/+page.svelte     Item detail
    settings/+layout.svelte     settings sub-nav
    settings/+page.svelte       General (stub)
    settings/libraries/+page.svelte
    settings/dashboard/+page.svelte (stub)
    settings/logs/+page.svelte  Logs (the original PoC target)
```
