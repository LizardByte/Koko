# Porting guidelines — Svelte 5 client

Rules for porting vanilla client code (`../client-web/src/`) into this Svelte 5
client. Read alongside `POSTMORTEM.md` (the failure mode these rules prevent).

The governing principle: **the vanilla client is the fidelity contract.** Port
its behaviour and appearance exactly; diverge only with a recorded reason.

---

## 1. CSS: scoped vs global — one source of truth per rule

A CSS rule lives in **exactly one** place. Never two. Drift begins the moment
a second copy exists.

**Global (`src/app.css`)** — a rule belongs here when *any* of these hold:
- It's a design token, `:root` variable, or element default (`button`, `input`,
  `a:focus-visible`).
- It's a cross-cutting utility (`.panel`, `.page-panel`, `.tag` + variants,
  `.muted`, `.eyebrow`, `.label`, `.empty-state`, `.loading-spinner`).
- **It is used by 2 or more components.** Examples currently global because
  multiple components render them:
  - `.hero-meta-row` — ItemHero, PersonHero, HomeFeature, ItemSupport
  - `.item-grid` — HomeContent, ItemChildren
  - `.media-card*` family — MediaCard, HomeContent's collection cards
  - `.detail-art`, `.detail-summary`, `.detail-actions`, `.hero-tagline`,
    `.item-hero`, `.item-poster`, `.item-summary`, `.item-title-fallback` —
    ItemHero + PersonHero
  - `.brand-block`, `.brand-mark`, `.brand-logo` — Rail, AuthShell, +layout
  - `.icon-button`, `.shelf-row`, `.extras-row`, `.people-row` — shared layout

**Scoped (component `<style>`)** — a rule belongs here when it is owned by a
**single** component. Examples:
- ItemPeople: `.person-card`, `.person-card-art`, `.person-card-title`, …
- ItemExtras: `.media-extra-card`, `.media-extra-thumbnail`, …
- ItemSupport: `.item-support-grid`, `.item-info-list`
- ItemHero: `.item-hero.episode-hero`, `.item-thumbnail`, `.item-title-logo`,
  `.item-fact-list`, `.item-fact`
- CollapsibleText: `.collapsible-text`, `.text-toggle-button` (the component
  is reused; its rules travel with it — still single-owner)
- PersonCredits: `.person-credit-*`, `.person-season/episode-credit-grid`
- PersonHero: `.person-poster`, `.button-link`, `.person-hero`
- HomeContent: `.search-result-*`, `.home-tab-panel`, collection-card glue
- HomeFeature: `.home-feature*`
- HomeNavbar: `.home-navbar`, `.browse-tab*`, `.search-form`, `.search-toggle-button`

**Decision procedure before adding any rule:**
1. Grep vanilla `style.css` for the selector. If it exists, the ported values
   **must match vanilla** unless you have a recorded reason.
2. Is the selector rendered by more than one component? → global.
3. Only one component? → scoped to that component.
4. Never add a scoped rule that duplicates a global rule. If a global rule
   exists, delete the scoped copy or vice-versa.

**Scoped rules that must reach child-component elements** need `:global` on
the child part, e.g. `.media-extra-placeholder-icon :global(svg)` (the `<svg>`
comes from `<Icon>`), or `:global(.home-feature-action)` (the class is on an
element rendered by `<Button>`). Plain `.parent svg` in a scoped block
silently fails to match — svelte-check flags these as "Unused CSS selector";
treat that warning as a real bug, not noise.

---

## 2. Icons

The vanilla client sizes icons via **CSS class context**, not attributes:

| Vanilla class | size | stroke-width |
|---|---|---|
| `.rail-icon svg`, `.card-icon svg`, `.brand-icon svg` | `1.1rem` (~18px) | 2 |
| `.button-icon svg` | `1rem` (16px) | 2.1 |
| `.status-icon svg` | `0.95rem` (~15px) | 2.2 |
| `.media-card-status.is-watched .status-icon svg` | `1.12rem` (~18px) | 2.2 |
| `.media-extra-placeholder-icon svg` | `2rem` (32px) | — |
| `.person-credit-tray-close svg` | `0.95rem` | — |

`@lucide/svelte`'s `size` prop writes `width`/`height` **attributes**, which
override CSS. So:

- **Prefer letting CSS size the icon.** Render the icon inside the wrapper
  vanilla expects (`.rail-icon`, `.card-icon`, `.button-icon`, `.status-icon`,
  `.media-extra-placeholder-icon`) and pass **no `size`** (or the exact rem→px
  equivalent). The global CSS in `app.css` carries these sizing rules.
- **Only pass an explicit `size`** when there is no vanilla CSS context for
  the slot (e.g. the mock-mode `.media-card-art-fallback` placeholder).
- **Icon name fidelity.** Use the *exact* name vanilla's `renderIcon('…')`
  uses for the same UI slot. The `house`/`layout-grid` substitution was a real
  bug. When porting a new view, list its `renderIcon` calls and use the same
  names.
- **Register before use.** Any icon name must exist in
  `src/lib/components/Icon.svelte`'s `ICONS` map, or it renders nothing. Add
  the import + map entry in the same change that first uses it. (The player
  and settings icons are pre-registered for the upcoming phases.)

---

## 3. Class lists are part of the port

When porting a vanilla `render*` string template, **copy the class attribute
verbatim**. Do not add or remove classes by feel.

- Vanilla `class="icon-button search-toggle-button"` ⇒ the Svelte button must
  have exactly those classes. Adding `secondary-button` flipped the colour.
- Vanilla `renderButtonContent('Open', 'arrow-right')` defaults
  `iconPosition` to `'start'` ⇒ the `<Button>` must not pass
  `iconPosition="end"`.
- The `<Button>` component forwards `class` — use it to attach vanilla
  positioning classes like `home-feature-action`.

When in doubt, diff the rendered `class="…"` string against vanilla's
template output.

---

## 4. Routes and navigation

- Every `goto('…')` target **must** have a matching directory under
  `src/routes/`, or be a documented browse-filter store action (see the B6
  proposal once decided).
- No inline path strings without a route check. A dead `goto` 404s silently.
- Vanilla's `browseFilter` (collection/category/playlist detail) is **not** a
  route in vanilla — it sets `state.browseFilter` and re-renders home. Until
  the B6 decision is made, do not point links at `/collections/:id`.

---

## 5. Markup structure is a contract

Icon wrappers are part of the CSS contract. Don't drop them when swapping
`renderIcon('x', 'card-icon')` for `<Icon name="x" />`:

- Vanilla: `<span class="media-card-kind"><span class="card-icon"><i …></span></span>`
- The `.card-icon` span is what `.card-icon svg { width: 1.1rem }` targets.
  Omit it and the icon renders at the wrong size.

Always reproduce the wrapper spans vanilla emits (`.card-icon`, `.status-icon`,
`.library-refresh-indicator`, `.media-extra-placeholder-icon`, …).

---

## 6. Interaction parity

Vanilla's event bindings (`eventBindings.ts`) encode behaviours that aren't
obvious from the render code. When porting a clickable element, check the
vanilla handler reproduces:

- **Tab switch clears search + browse filter** (not just sets the tab).
- **Live search on input** with a 250ms debounce (not submit-only).
- **Scan / refresh-metadata** set busy state and re-fetch home/items (Phase 6
  will wire the polling; for now at least call the store action).
- **Person Back** uses `history.back()`, not `goto('/')`.
- **Collapsible text** toggle state survives re-renders (store-backed).

---

## 7. Per-component verification checklist

Before declaring a component ported:

1. **Side-by-side.** Vanilla on `:5173`, Svelte on `:4174`, same route, same
   mock data. Compare at normal + narrow widths.
2. **Value diff.** For every scoped `<style>` rule, confirm the value matches
   vanilla `style.css`. (svelte-check's "Unused CSS selector" warning = a
   scoped rule that can't match; fix or remove.)
3. **Icon check.** Each icon: right name, right wrapper class, right size.
4. **Class-list check.** Rendered `class="…"` matches vanilla template.
5. **Route check.** Every `goto` target resolves.
6. **`svelte-check` clean** (0 errors; warnings reviewed, not just ignored).
7. **`cargo +nightly fmt`** (per `AGENTS.md`) before commit.

---

## Out of scope for these guidelines (tracked separately)

- Player / playback UI (separate `playbackController` spike; ~550 lines of
  vanilla CSS not yet ported).
- Full Settings surface CSS (`.settings-drawer`, `.metadata-dashboard-*`,
  `.settings-library-card`, …) — Phase 5.
- `@media (max-width: 1320px)` and the full `@media (max-width: 960px)` block
  (rail-as-bar, workspace-grid, player responsive) — Phase 6/7.
- Browse-filter mechanism (collection/category/playlist detail) — B6 proposal.
