# Proposal: Rewrite the Koko web client in Svelte 5 (+ optional Tauri desktop shell)

**Status:** Proof-of-concept / proposal — not a merged plan. Prepared for discussion.
**Scope:** `crates/client-web` (the web UI). No backend changes are required or proposed.
**Worktree:** `poc/svelte-rewrite` (companion PoC in `crates/client-web-svelte/`).

---

## TL;DR

1. **Rewrite `client-web` from vanilla TypeScript into Svelte 5**, shipped as a static SPA (SvelteKit with `adapter-static`, SSR disabled). The rewrite is well-supported, modest effort, and the single biggest win is **deleting the hand-rolled `domPatcher.ts`** (~290 lines) and most of `eventBindings.ts` (~1,395 lines) — Svelte's reactivity replaces them natively.
2. **A Tauri desktop shell is viable and lower-risk than typical** because Koko's server already builds on `tao` + `tray-icon` + `keyring` — the exact crates Tauri is made of. Tauri is an *optional follow-on*, not a prerequisite. The recommended architecture keeps the HTTP-served SPA for remote clients (phones, tablets, LAN TVs) **and** adds a Tauri shell that loads the same frontend for desktop.
3. **The real risk is `playbackController.ts`** (video/audio/YouTube trailers), not the framework swap. That file needs careful, incremental porting. The logs-view PoC in this worktree de-risks everything *except* playback.
4. **Recommendation:** pursue the Svelte rewrite incrementally (view-by-view, keeping the vanilla client buildable in parallel), and treat the Tauri shell as a separate, later decision once the rewrite has shipped.

---

## 1. Why consider this at all

The current client is a **vanilla TypeScript SPA** (~11,750 lines across `src/app/`) bundled with Vite. It has exactly one runtime dependency (`lucide`). It is genuinely lean, and that's worth preserving.

However, it implements its own UI framework by hand:

- **`domPatcher.ts`** — a custom DOM reconciler with keyed reordering, focus/scroll/selection snapshot+restore, form-control syncing, and a special-case guard so `<video>`/`<audio>` elements don't reload mid-render.
- **`eventBindings.ts`** — 1,395 lines that wires up every DOM event by hand, including a temporary monkeypatch of `EventTarget.prototype.addEventListener` to auto-scope listeners to a per-render `AbortController`.
- **`app.ts`** — an 865-line orchestrator that does full-app string-template re-renders on essentially every state change, with hand-rolled snapshot-diffing to suppress redundant renders.

This is a lot of bespoke machinery solving problems that a reactive framework handles natively. It's not broken — but it's a long-term maintenance liability: every new feature has to thread through the render/event-binding lifecycle correctly, and the patcher's quirks (focus preservation, media-element src stability) are the kind of thing that breaks silently.

A rewrite to Svelte 5 replaces ~1,700 lines of that glue (`domPatcher` + most of `eventBindings`) with idiomatic, framework-maintained equivalents, while keeping the bundle small (Svelte compiles away; the PoC's whole app is **132 KB** of static output).

---

## 2. The PoC — what it proves

A minimal Svelte 5 port of the **Settings → Logs** view lives in `crates/client-web-svelte/`. It was chosen because it's small, self-contained, and exercises the three architectural seams that matter most: state, rendering, and events — without touching playback.

**Validated by the PoC:**

| Claim | Evidence |
|---|---|
| Static SPA build the Rust server can serve | `npm run build` → `dist/index.html` + `dist/_app/` (132 KB total), using `adapter-static` with SPA fallback |
| Routing parity for `/settings/logs` | File-based route `src/routes/settings/logs/+page.svelte`; `/` also resolves |
| Data layer ports verbatim | `getLogs` + `LogEntriesResponse`/`LogEntry` types + `VITE_USE_MOCK_API` toggle copied from the vanilla client unchanged; mock mode active |
| Dev workflow carries over | `npm run dev:mock` (= `vite dev --mode mock`) serves at `http://127.0.0.1:4173/` with mock data, identical to the vanilla client's fixed workflow |
| `domPatcher`/`eventBindings` disappear | The logs view's filter form + table + refresh/clear handlers are one ~150-line `.svelte` file with local `$state` runes — no patcher, no AbortController, no FormData-rehydration |
| The Message-column fix ports for free | `.log-message-col { min-width: 70ch }` is carried into the PoC's CSS unchanged |

**How to run it:**
```bash
cd crates/client-web-svelte
npm install
npm run dev:mock     # http://127.0.0.1:4173/  → Settings → Logs
```

**What the PoC deliberately does NOT cover:** `playbackController.ts`, `youtube.ts`, the home shelves' lazy-loading/virtualization, `itemPersonView`, and the auto-refresh polling in `app.ts`. Those are the real work of a full migration (see §4).

---

## 3. Tauri — the surprising finding

The most important discovery from the research: **Koko's server already uses Tauri's own lower-level crates.**

| Koko today (`crates/server`) | What it is | Tauri equivalent |
|---|---|---|
| `tao` (event loop / windowing) | The Tauri team's windowing library | Tauri's windowing layer |
| `tray-icon` | The Tauri team's standalone tray library | `tauri::tray::TrayIconBuilder` |
| `keyring` / `keyring-core` (in `secrets.rs`) | OS keychain access | What Tauri's recommended keychain plugins wrap |

`crates/server/src/tray.rs` builds the tray menu (Open / Donate / Quick Options / API Docs / About / Quit) directly on `tao`'s `EventLoop`. Tauri = `tao` + `wry` (webview) + a command/IPC/plugin layer on top. **Koko is already ~80% of the way to Tauri at the crate level** — it's just missing `wry` and Tauri's command framework.

### What this means concretely

- **Tray migration is mechanical.** `tray.rs` ports ~1:1 onto `TrayIconBuilder` because it's the same upstream crate. Lowest-risk part.
- **Don't adopt a Tauri secret plugin.** Koko's `secrets.rs` already does OS-keychain access via `keyring`, which is strictly better than `tauri-plugin-stronghold` (no master password, OS-managed) and avoids a component the Tauri team has flagged for deprecation. Keep `secrets.rs` as-is.
- **Tauri does NOT bundle Chromium.** It uses the OS native WebView: WKWebView (macOS), WebView2 (Windows), WebKitGTK (Linux). So "rendering on Tauri" just means "the web client in a native window" — your browser baseline carries over.

### The three process-model options for the Rust server + Tauri

1. **In-process (embed the server as a crate).** Koko's server already exposes a `lib.rs`, so Tauri's `.setup()` hook can spawn it (axum/tokio task) in the same process. Zero IPC for server logic; frontend still hits `localhost` over HTTP, or migrates gradually to Tauri `invoke`. Cleanest end-state; the footgun is co-hosting the HTTP server and Tauri on one tokio runtime (documented, solvable).
2. **Sidecar (bundle the standalone `koko` binary).** Tauri spawns it as an external process. Preserves "server runs standalone for headless/remote use." More moving parts (lifecycle, port discovery, two binaries to sign).
3. **Hybrid (HTTP only).** Tauri is purely a window shell that loads the remote/local server URL. Smallest change; loses the "single binary" story.

**Recommended:** treat the process-model choice as a *separate* decision after the Svelte rewrite has shipped. Options 1 and 2 are both viable; the right answer depends on packaging preferences, not on the frontend framework.

### The one real Tauri risk: Linux / WebKitGTK

On Windows and macOS, the native-WebView floor is effectively "recent Chromium / recent Safari" — nearly free. On Linux, Tauri uses the distro's `webkit2gtk-4.1`, which on stable/enterprise distros can be years behind and has known rendering bugs (glitchy maximize, font-weight, NVIDIA/Arch blank windows). **Mitigation: target Linux via Flatpak**, which bundles a known `webkit2gtk` — Koko already ships a Flatpak, so this fits.

---

## 4. Migration risk ranking (per file)

From a full read of every module in `src/app/`. Tiers: **T** trivial, **M** moderate, **H** hard.

### Trivial — port nearly mechanically
- `api.ts` (1,285) + `mockApi.ts` (1,590) — copy verbatim. Framework-free data layer. **The PoC proves this.**
- `format.ts`, `constants.ts`, `providers.ts`, `mediaExtras.ts`, `playbackProgress.ts`, `mediaTargets.ts`, `activities.ts`, `selectors.ts` (357) — pure functions over `state`; become `$derived` / helpers.
- `types.ts` — keep as-is.

### Moderate — real logic, no exotic coupling
- `routes.ts` (63) → SvelteKit file routes. URL shapes must be preserved exactly.
- `state.ts` + every `render()` call site → `$state` runes / stores. Conceptually easy; the work is finding every call site.
- `settingsView.ts` (831), `auth.ts` (168), `ui.ts` (162), `dashboardView.ts` (332 — **logs lives here**), `homeView.ts` (1,014), `input.ts` (80), `formUtils.ts`.

### Hard — the meaty seams
- **`domPatcher.ts` (290) — good-news-H.** The most novel code in the codebase. **Deleted entirely** in Svelte; Svelte's reactivity + `{#each key}` replace it. Risk is *verifying* nothing depends on its quirks beyond what Svelte covers — specifically the `<video>`/`<audio>` src-stability guard and the lazy-shelf `beforePatch` hook.
- **`eventBindings.ts` (1,395) — H for effort, not novelty.** ~90% becomes inline Svelte handlers; the `addEventListener` monkeypatch and the `AppEventBindingContext` seam disappear. Risk: subtle behaviors (trailer long-press chooser, escalating seek, deferred renders) live here.
- **`playbackController.ts` (1,385) — genuinely hard. THE headline risk.** Owns the `<video>`/`<audio>` player + two YouTube iframe players, imperatively: module-level mutable refs, escalating-seek handlers, 500ms progress polling, fullscreen/PiP, audio-track switching that triggers remux via `render(false)`, autoplay-block handling. Porting means a `<VideoPlayer>`/`<TrailerPlayer>` component with `onMount`/`onDestroy` lifecycle and `$state` for play state. **This is the single biggest chunk of effort and the most likely to harbor bugs.**
- **`youtube.ts` (115) — H-ish, small but exotic.** The URL→videoId parser ports as-is; the YouTube IFrame API loader (`onYouTubeIframeAPIReady` global + script injection) needs a Svelte-friendly singleton wrapper.

### The orchestrator
- `app.ts` (865) — the migration spine. `startApp` → `+layout.svelte`; `render()` → component reactivity; `refreshData`/`refreshPending*` → `load` functions + polling stores; `navigateTo` → `goto()`. Rewritten last.

---

## 5. Browser compatibility impact

Svelte 5 requires ES Proxies and modern browser APIs. Per the [official Svelte browser-support table](https://svelte.dev/docs/svelte/browser-support), minimums are roughly **Chrome/Edge 91+, Firefox 90+, Safari 14.1+**. Vite/esbuild transpiles output to your `browserslist` target, so the *practical* floor is the Proxy requirement (Safari 12-ish if you really stretch it), not syntax.

For a media-server web UI in 2026, this is a non-issue for browser users. **The only genuine risk is very old Smart TV / embedded WebView engines** — worth checking actual user-agent stats before committing. (And if Koko ever wraps in Tauri, the WebKitGTK-on-stable-Linux concern in §3 applies.)

Net: **no realistic browser-users lost.**

---

## 6. Recommended architecture

```
                    ┌─────────────────────────────────┐
                    │   Rust server (crates/server)   │
                    │   serves static SPA over HTTP   │
                    └──────────────┬──────────────────┘
                                   │
              ┌────────────────────┼─────────────────────┐
              │                    │                     │
     Remote browsers          Tauri desktop shell    (headless /
     (phones, tablets,        loads same dist/       remote API clients
      LAN TVs)                + tray/window/IPC)
```

- **One frontend codebase**, built once to static `dist/`.
- The Rust server serves it over HTTP (status quo) for all remote clients — the load-bearing property for a media server.
- *Optionally*, a Tauri app embeds the same `dist/` for a native desktop experience (tray, single window, no port UX). This is an additive, later decision.
- Svelte 5 with **SSR disabled** (the PoC's `+layout.ts`). SvelteKit's SSR/load-functions/endpoints add no value when there's no Node runtime — only inside Tauri or served statically.

---

## 7. Svelte 5 (library) vs SvelteKit (meta-framework)

A real fork in the proposal:

- **SvelteKit + `adapter-static`** — file-based routing, `load` functions, first-class Tauri template. Imposes its conventions (`+page.svelte`, `+layout.ts`). The PoC uses this.
- **Plain Svelte 5 + a tiny router** — closer 1:1 port of Koko's existing `routes.ts`; less to learn; but you hand-roll routing and miss SvelteKit's dev ergonomics.

**Recommendation:** SvelteKit in SPA-only mode. The conventions are mild, the Tauri story is first-class, and `goto()`/`$page` replace Koko's hand-rolled `navigateTo`/`parseRoute`. Disable SSR globally (one line, already in the PoC).

---

## 8. Effort & sequencing

**Rough sizing:** weeks-to-a-few-months for one focused dev, dominated by the view modules and `playbackController.ts` — not a multi-quarter effort. The data layer, types, and pure helpers are essentially free (copy + `$derived`).

**Recommended sequence (incremental, vanilla client stays buildable in parallel):**

1. **Scaffold** — SvelteKit SPA alongside `client-web` (this PoC). Wire CI to build both.
2. **Data + types** — copy `api.ts`, `mockApi.ts`, `types.ts`, pure helpers. (PoC-level: done for logs.)
3. **Auth shell + routing** — `+layout.svelte`, login/welcome, route parity for all `routes.ts` shapes.
4. **Settings views** (logs ✓, dashboard, providers, libraries, scheduled) — lowest-risk real views.
5. **Home + item/person views** — shelves, lazy-loading, metadata search. Re-implement lazy-shelf virtualization.
6. **Playback** — `playbackController.ts` + `youtube.ts` last, as dedicated `<VideoPlayer>`/`<TrailerPlayer>` components. Highest risk; budget the most time and keep the vanilla player available as a fallback during this phase.
7. **Delete the vanilla client** once parity is verified.

**Tauri** is a separate workstream after step 6, gated on its own decision.

---

## 9. Decision asks

1. **Agree to pursue the Svelte 5 rewrite incrementally** (vanilla client kept buildable in parallel)?
2. **SvelteKit SPA vs plain Svelte + router** — preference? (Recommendation: SvelteKit SPA.)
3. **Should a `playbackController.ts` spike happen early** to de-risk the headline item before committing to the full sequence?
4. **Tauri** — defer to a post-rewrite decision, or explore in parallel? (Recommendation: defer; the rewrite unblocks it but doesn't require it.)
5. **Linux WebKitGTK strategy** — confirm Flatpak is the supported Linux desktop distribution (it already exists), so a future Tauri shell has a controlled WebView version.

---

## 10. Sources

**Svelte / browser support:**
- [Svelte browser support docs](https://svelte.dev/docs/svelte/browser-support)
- [SvelteTalk — Safari & older targets](https://sveltetalk.com/posts/svelte-5-browser-support-safari-older-targets)
- [Reddit — Svelte 5 vs v4 support](https://www.reddit.com/r/sveltejs/comments/1di1oan/is_browser_support_the_same_in_svelte_v5_vs_v4/)

**Tauri (official):**
- [Webview Versions — Tauri v2](https://v2.tauri.app/reference/webview-versions/)
- [Tauri 2.0 release blog](https://v2.tauri.app/blog/tauri-20/)
- [SvelteKit frontend guide — Tauri v2](https://v2.tauri.app/start/frontend/sveltekit/)
- [Calling Rust from the Frontend — Tauri v2](https://v2.tauri.app/develop/calling-rust/)
- [System Tray — Tauri v2](https://v2.tauri.app/learn/system-tray/)
- [Embedding External Binaries (sidecar) — Tauri v2](https://v2.tauri.app/develop/sidecar/)
- [Stronghold plugin — Tauri v2](https://v2.tauri.app/plugin/stronghold/)

**Tauri (issues / community):**
- [Stronghold deprecation discussion #7846](https://github.com/orgs/tauri-apps/discussions/7846)
- [Tauri v2 constrained Linux compatibility #9039](https://github.com/tauri-apps/tauri/issues/9039)
- [Glitchy rendering on Linux #13157](https://github.com/tauri-apps/tauri/issues/13157)
- [Bundle chromium renderer request #14963](https://github.com/tauri-apps/tauri/issues/14963)
- [SvelteKit scaling discussion sveltejs/kit #13455](https://github.com/sveltejs/kit/discussions/13455)
- [Datawrapper: migrating to SvelteKit](https://www.datawrapper.de/blog/migrating-our-web-app-to-sveltekit)
- [Firezone: using Tauri (AppImage bundles webkit)](https://www.firezone.dev/blog/using-tauri)

**Repo evidence (read directly):**
- `crates/server/Cargo.toml` — `tray`/`native-secret-store` features; `tao`, `tray-icon`, `keyring`, `keyring-core` deps
- `crates/server/src/tray.rs` — tao + tray-icon event loop
- `crates/server/src/secrets.rs` — keyring-based OS keychain
- `crates/client-web/src/app/domPatcher.ts` — custom reconciler (290 lines)
- `crates/client-web/src/app/eventBindings.ts` — manual event wiring (1,395 lines)
- `crates/client-web/src/app.ts` — render orchestrator (865 lines)
- `crates/client-web/src/app/dashboardView.ts:260-332` — `renderLogViewer()` (PoC source)
- `crates/client-web/src/api.ts:1193-1223` — `getLogs()`; `:581-593` — `LogEntry`/`LogEntriesResponse`
- `crates/client-web/src/mockApi.ts:1046-1097` — `getMockLogs()` (PoC mock data)
