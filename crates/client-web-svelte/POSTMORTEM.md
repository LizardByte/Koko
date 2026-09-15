# Postmortem — Phase 1–4 fidelity drift

Scope: the Svelte 5 port of the Koko web client, Phases 1–4 (app shell +
auth, home, item detail, person detail). Written after a full source-level
diff against the vanilla client (`../client-web/`) surfaced ~90 distinct
fidelity regressions and they were fixed.

## Summary

Phases 1–4 shipped with widespread visual and behavioural drift from the
vanilla reference. The port was structurally sound (stores, types, mock data,
API layer all faithful) but the **presentation layer** had diverged in
dozens of small, individually-invisible ways that compounded into a UI that
"felt off" side-by-side. Root cause was not a single mistake but a pattern.

## What the drift looked like (representative samples)

- **Phantom refresh rings** on Movies & Shows in the rail. The rail showed a
  metadata-refresh spinner next to those two libraries because the gating
  condition (`metadata_refresh_total > 0`) was looser than vanilla's
  (`total > 0 && pending > 0`, or an active activity). Music showed nothing
  because its items lack `has_metadata`, which made the bug look deliberate.
- **Wrong Home glyph + oversized nav icons.** The rail used `layout-grid`
  where vanilla uses `house`, and every rail icon was `size={24}` where
  vanilla CSS sizes `.rail-icon svg` to `1.1rem` (~18px).
- **Grey search button.** The search-toggle gained an extra `secondary-button`
  class that vanilla omits, flipping it from the brand gradient to translucent
  grey.
- **`[Open →]` instead of `[→ Open]`.** The HomeFeature Open button passed
  `iconPosition="end"` where vanilla's `renderButtonContent('Open',
  'arrow-right')` defaults to start.
- **Person cards restyled.** `.person-card-art` font-size `1.4rem` vs vanilla
  `2.2rem`, blue gradient vs flat `rgba(255,255,255,.08)`, wrong subtitle size.
- **Item facts, extras, support grid, collapsible "see more", search-result
  rows** — all had hand-adjusted values that drifted from vanilla's.

## Root cause

**No fidelity checkpoint between porting a component and moving on.** Each
phase copy-pasted a rule from vanilla `style.css` into a scoped `<style>`
block, then "tidied" it — changed a gap, swapped a gradient, added a class,
substituted an icon — and nothing caught the change. Five compounding
factors:

1. **Two copies of every rule.** Vanilla has one global `style.css`; the port
   re-declared rules in per-component scoped blocks. The scoped copy *looked*
   authoritative (Svelte highlights it, the component "owns" it), so the
   global/vanilla reference was easy to forget.
2. **Svelte scoping hid the divergence.** A scoped rule only applies to the
   component's own elements, so a drifted value wouldn't show up elsewhere —
   it just silently made that one component wrong.
3. **Icons bypassed CSS sizing.** The `@lucide/svelte` `size` prop writes
   `width`/`height` attributes that override CSS, so passing `size={24}`
   defeated vanilla's `.rail-icon svg { width: 1.1rem }`. There was no signal
   that a CSS rule was being ignored.
4. **Class-list changes were invisible.** Adding `secondary-button` or
   `iconPosition="end"` is a one-token edit in Svelte vs a string-template
   concatenation in vanilla; nothing diffed the rendered class lists.
5. **Dead routes failed silently.** `goto('/collections/:id')` pointed at a
   route that doesn't exist; it just 404'd on click with no compile-time or
   render-time warning.

## What went well

- The **non-presentation layers are faithful**: store split, API type surface,
  mock seed data (verbatim), the four behavioural quirks, URL builders. These
  survived the diff with zero changes.
- The **mock-mode gradient fallbacks** on `MediaCard` (`.fallback-0`…`.fallback-4`)
  are a good, deliberate addition with no vanilla counterpart — kept as-is.
- The audit was **source-grounded**, not screenshot-guessed. Every fix maps to
   a specific `file:line` in vanilla.

## What we changed (this pass)

- **Surgical fixes:** Rail refresh-ring gating + Home glyph + icon sizes + `rail-avatar`
  class; HomeFeature Open button order + `home-feature-action` class; HomeNavbar
  search-toggle `secondary-button` removed; missing icons added to `Icon.svelte`
  (`house` + the player/settings set so later phases don't render blanks).
- **Wiring fixes:** tab-switch clears search/browse-filter; debounced
  live-search-on-type (250ms); person Back uses `history.back()`.
- **Value restoration:** every drifted scoped value corrected to vanilla
  (person-card, media-card status family, item facts, extras, support grid,
  collapsible text, person credits, search-result rows with restored
  thumbnails + person/playlist result types, home-feature, navbar gap).
- **Structural markup fixes:** MediaCard wraps the kind icon in `<span
  class="card-icon">`, emits a single `has-multiple` span when both
  unmatched+pending (vanilla semantics), restores watch-count + progress% +
  icons to item-detail tags; ItemExtras uses the `media-extra-placeholder-icon`
  class and `music` for theme songs.
- **Policy:** see `PORTING_GUIDELINES.md` — the scoped-vs-global line is now
  written down so Phases 5–7 don't repeat this.

## Lessons

1. **The reference stylesheet is the contract.** When porting, read the
   vanilla rule verbatim; if you change a value, you need a reason and a note.
2. **One source of truth per rule.** A rule lives in *exactly one* place —
   either global (shared) or scoped (single-component) — never both. Drift
   begins the moment a second copy appears.
3. **Icons: let CSS size them where vanilla does.** Don't pass `size` unless
   vanilla has no CSS sizing context for that slot.
4. **Class lists are part of the port.** Copy them verbatim from the
   `render*` template; don't add/remove by feel.
5. **Every `goto` needs a route.** No inline path strings without a matching
   `routes/` directory or a documented browse-filter action.
6. **Verify side-by-side before declaring a phase done.** Two dev servers,
   same route, same mock data — every time.
