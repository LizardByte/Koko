# Transcode: FFmpeg Resolution & Player Error Surfacing — Design

- **Date:** 2026-06-18
- **Status:** Approved (design), pending implementation plan
- **Scope:** A focused, timer-free fix for two reported failures plus adjacent robustness gaps. Lifecycle/mid-stream handling is deliberately deferred (drafted in the appendix).

## 1. Background & Root Cause

Two failures were reported when playing an HEVC (transcode-required) item:

1. **"Failed to spawn transcode: No such file or directory (os error 2)"** — transcoding never starts.
2. **Web player stays on a black screen with the "Transcoding" badge/spinner** — no error surfaced.

### 1.1 Root cause of issue 1 (the "spaces in path" framing is a red herring)

FFmpeg arguments are built into a `Vec<String>` and passed via `tokio::process::Command::args(&args)` (`crates/server/src/transcode.rs:227-229`). This bypasses the shell entirely — each vector element is handed directly to the OS `exec` as a discrete argv entry. A source path containing spaces or brackets (e.g. `/Users/.../Witch Hat Atelier ....mkv`) is passed to ffmpeg verbatim. The unquoted appearance in the server log is purely cosmetic: it comes from `args.join(" ")` at `transcode.rs:222-225`, not from what is executed. We could fix that, but that's not the real issue.

`No such file or directory (os error 2)` returned by **`spawn()` itself** (not by ffmpeg later) is, on Unix, `ENOENT` from the exec layer. When spawning the *command* fails with `ENOENT`, it means **the executable was not found** — i.e. `ffmpeg` is not resolvable from the server process's environment.

The decisive context: `ffmpeg_path` defaults to a bare `"ffmpeg"` (`crates/server/src/config.rs:141`). On macOS, applications launched as GUI `.app` bundles (the recent DMG packaging work, commits `d8a5ea3` / `ea83ef9`) **do not inherit the user's shell `PATH`**. So even though ffmpeg is installed at `/opt/homebrew/bin/ffmpeg` and works from a terminal, it is invisible to a bare `ffmpeg` lookup from the server process. The same resolution is used for capability detection at `crates/server/src/media.rs:5997` (`detect_binary`).

Note: a *missing media file* would not produce `ENOENT` at spawn. ffmpeg would start, fail to open its input, and emit the error on stderr (which the code already captures). The fact that the process dies at spawn points squarely at executable resolution.

### 1.2 Root cause of issue 2 (no player feedback on HTTP failure)

When `/api/v1/sessions/<id>/stream` returns `500`, the `<video>` element receives a non-2xx media resource. Browser behavior here is notoriously inconsistent: the `error` event fires reliably for *decode* failures but **not** for HTTP failures on a media `src`. In practice the element often just stalls on a black screen with the loading spinner — exactly what was observed.

The player already has an error indicator (`has-media-error` toggling `.player-error-indicator`, `crates/client-web/src/app/playbackController.ts:324-327`), but:
- Its copy is hardcoded ("Playback could not start / Try another audio track or start playback again.").
- It is driven only by the `<video>` `error` event (`playbackController.ts:1330-1333`), which doesn't fire for HTTP 5xx on the src.
- The capability data (`capabilities.transcoding.ffmpeg.available`) is loaded into state (`app.ts:536`) but never consulted before playback.

### 1.3 Adjacent gaps found during exploration

| ID | Gap | Evidence |
|----|-----|----------|
| B | Spawn failures are reported opaquely | `media.rs:3651-3654` logs `"Failed to spawn transcode: {}"` and returns bare `Status::InternalServerError` (no body) |
| C | ffmpeg startup diagnostics are dropped before failure is known | `transcode.rs:228-233` pipes stderr but only reads it inside a background task *after* returning the stream; route returns 200 with a dead stream when ffmpeg fails to open input |
| E | No source-path existence check | transcode branch of `get_session_stream` (`media.rs:3574-3617`) has no existence check |
| F | No tests around transcode arg generation / path handling | no `transcode` test module exists |

## 2. Decisions (locked during brainstorming)

- **Locate ffmpeg:** both — search well-known locations (self-healing) **and** strong detection/messaging. Manual path still allowed, with a radio picker for easy setup.
- **Discovery timing:** on-demand from settings ("Detect ffmpeg"), and that call must *also validate the user's currently-configured path*, not only search.
- **Settings picker:** pair ffmpeg + ffprobe by directory (one radio per directory containing both).
- **Player failures:** structured stream error (server) + client watchdog. Watchdog/timed pieces are **deferred** (appendix); this spec ships timer-free.
- **Scope:** ship the timer-free core now; capture all lifecycle discoveries as a draft so nothing is relearned.
- **Error UX:** layered — the in-player overlay gets terse, player-appropriate copy; the global `error-panel` banner gets the actionable layer (e.g. an "Open settings" button) when the error's `action` warrants it.
- **Barebones seams:** add two inert seams now (per-session error store + shared error-mapper with `action`) so the deferred lifecycle phase has its hooks without this spec gaining timers or behavior.
- **Tests:** shim-based (runs everywhere) for resolver + transcode-arg generation; a real-ffmpeg smoke test behind an optional feature flag.

## 3. Out of scope (deferred)

All of the following are **drafted in Appendix B** as a discovery + plan, not built in this phase:

- (C) first-byte / early-exit validation of the stream before committing.
- (C) mid-stream watcher + the `setInterval` client watchdog for stalls.
- (E) source-path existence check (also folds into the lifecycle phase).
- SSE / WebSocket transport (evaluated and rejected for this scope; see §6.2).

## 4. Design

### 4.1 FFmpeg resolution — new module `crates/server/src/ffmpeg_resolve.rs` (fixes issue 1)

A single source of truth for resolving an executable, used by **both** capability display and the actual spawn — so "available in UI" ⟺ "actually launches".

```rust
pub enum ResolveSource { ConfiguredAbsolute, PathLookup, WellKnown }

pub enum ResolvedBinary {
    Found   { resolved_path: PathBuf, version: Option<String>, via: ResolveSource },
    Missing { configured: String, checked_paths: Vec<PathBuf> },
}

pub fn resolve_binary(name_or_path: &str, binary_name: &str) -> ResolvedBinary;
pub fn resolve_ffmpeg(configured: &str) -> ResolvedBinary { resolve_binary(configured, "ffmpeg") }
pub fn resolve_ffprobe(configured: &str) -> ResolvedBinary { resolve_binary(configured, "ffprobe") }
```

**Resolution order:**
1. If `configured` is absolute and exists and is executable → use it (`ConfiguredAbsolute`).
2. Else if the bare name resolves on `PATH` → use it (`PathLookup`).
3. Else probe a platform-specific well-known list for `binary_name`:
   - macOS/Linux: `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`, `/bin`
   - Windows: standard install dirs under `Program Files\ffmpeg`, plus `%USERPROFILE%\bin`
   First hit → `WellKnown`.
4. Else → `Missing { configured, checked_paths }` enumerating everywhere looked.

**Integration:**
- `spawn_transcode` / `spawn_transcode_stdout` (`transcode.rs:184`, `:214`) call the resolver and execute the **resolved** absolute path. This is the actual fix for issue 1.
- `detect_binary` (`media.rs:5997`) is refactored to delegate to the resolver, preserving the existing `BinaryCapability` return shape so `TranscodingCapability` stays wire-compatible.

**Logging:** at startup and on each resolve, `INFO`/`WARN` naming the resolved path and source, or all `checked_paths` when missing.

**Notes:**
1. The user inquired if there's difference in figuring out if the `configured` is a bare name, or not, and the impact of having different treatment for a path or a filename, he is not sure the distinction is meaninful, but also unsure it isn't. Consider it and state your recommendations.
2. Homebrew may have different paths for arm and x86, so we need to consider that. Also, other popular package managers that we should have the well-known paths listed with lower priority (just further down the list, no need to create a priority property), both macOS/Linux?
3. Any popular package managers for Windows that may have a different path of installation than `Program Files\ffmpeg` and `%USERPROFILE%\bin` that we may want to include?
4. Should we enforce a hardcoded binary names allow-list that `resolve_binary` consumes to avoid potential attacks leveraging some vulnerability in the future? The user is concerned some mistake could expose use to a bunch of problems from path lookups via this method.

### 4.2 Discovery endpoint + settings UI (the "easy setup")

**Server — new route in `crates/server/src/web/routes/media.rs` next to `get_server_capabilities` (`media.rs:3026`), registered in `crates/server/src/web/routes/mod.rs`.** Tag is `Media` (there is no `system` tag in the project); guard is `AdminGuard` (the project's admin request guard, as in `user.rs:144/169`), since the response reveals on-disk paths and binary versions — the same sensitivity class as settings.

```rust
#[openapi(tag = "Media")]
#[post("/api/v1/system/tools/discover")]
pub async fn discover_transcoding_tools(admin_guard: AdminGuard) -> Result<Json<ToolDiscoveryResponse>, Status>;
```

Response types use the project style (`#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]` + a doc comment per field, as in `BinaryCapability` at `media.rs:115`):

```rust
pub struct ToolDiscoveryResponse {
    pub configured_ffmpeg: BinaryProbe,     // validation of the user's current path
    pub configured_ffprobe: BinaryProbe,
    pub candidates: Vec<ToolCandidate>,     // one per directory containing at least one
}
pub struct ToolCandidate {
    pub directory: String,                  // paths serialize as strings (JSON has no PathBuf)
    pub ffmpeg: Option<BinaryProbe>,        // None if absent in this dir
    pub ffprobe: Option<BinaryProbe>,
}
pub struct BinaryProbe {
    pub resolved_path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,              // e.g. "permission denied" / "exited non-zero"
}
```

**Behavior** (matches the locked decision): search `PATH` + the §4.1 well-known list **and** validate the currently-configured path by running `ffmpeg -version` / `ffprobe -version`, returning the probe result so the UI can show "your configured path works / doesn't". A `BinaryProbe` is the resolver's `Found`/`Missing` rendered for JSON. This endpoint mutates nothing — it is read-only discovery.

**Client — `crates/client-web/src/app/settingsView.ts`.** Under the existing ffmpeg/ffprobe inputs (`settingsView.ts:554-555`):
- A **"Detect ffmpeg"** button calls the discover endpoint.
- Results render as **radio options grouped by directory** (pair by directory): `/opt/homebrew/bin — ffmpeg 7.0 · ffprobe 7.0`. Directories missing one binary are shown but disabled, with a note. Selection sets **both** `ffmpeg_path` and `ffprobe_path` to that directory.
- The **currently-configured path** is its own radio option, labeled with its validation result ("✓ works" / "✗ not found: …").
- A manual-path radio is still available for the independent text inputs (preserving "still allow manual path").
- Saving still goes through the existing settings save — no new write path.

### 4.3 Spawn robustness & structured error reporting (gap B + half of issue 2)

**Richer spawn error.** `spawn_transcode_stdout` returns a new enum instead of `std::io::Error`:

```rust
pub enum SpawnTranscodeError {
    ExecutableMissing { checked_paths: Vec<PathBuf> },   // spawn ENOENT, from the resolver
    BadInput { ffmpeg_stderr: String },                  // populated by the lifecycle phase; left unused here
    Io(std::io::Error),
}
```

`BadInput` is present for forward-compatibility with the lifecycle phase but is not produced in this spec (no first-byte check — see out of scope).

**Shared error-mapping function (seam #2).** A single `map_transcode_error(SpawnTranscodeError) -> (Status, TranscodeErrorBody)` is written once so the lifecycle phase reuses it:

```rust
pub struct TranscodeErrorBody {
    pub code: &'static str,        // "transcode_executable_missing" | "transcode_input_error" | "transcode_failed"
    pub message: String,
    pub action: Option<&'static str>,   // e.g. Some("open_settings") for actionable codes
}
```

`action` is what lets the global banner conditionally render an "Open settings" button.

**Status mapping** (to be confirmed during implementation, not load-bearing for the design): `ExecutableMissing → 503`, `BadInput → 422`, `Io → 500`.

**Route handler** (`media.rs:3621-3655`): on spawn error, return the structured JSON body via the mapper (not a bare 500). `log::error!` is retained and now includes the resolved path and `checked_paths`.

**Per-session error store (seam #1, inert writer; live reader).** Added next to `ACTIVE_TRANSCODE_TASKS` (`media.rs:317`):

```rust
static ACTIVE_SESSION_ERRORS: Lazy<RwLock<HashMap<String, SessionError>>> = Lazy::new(Default::default);
pub async fn record_session_error(session_id: &str, error: SessionError) { /* unused in this phase */ }
```

The writer is inert (no callers this phase — the lifecycle watcher in Appendix B will populate it). A **lightweight read endpoint** is added now so the §4.4 best-effort recovery can look it up without spawning a transcode:

```rust
#[openapi(tag = "Media")]
#[get("/api/v1/sessions/<session_id>/status")]
pub async fn get_session_status(session_id: String) -> Json<SessionStatusResponse>;
// SessionStatusResponse { error: Option<TranscodeErrorBody>, ... } — error is None while this phase never writes any
```

This endpoint returns `{ error: None }` in this phase (since nothing writes the store yet) but is the seam the lifecycle watcher will feed. It is a map lookup, not a transcode spawn — that is the entire reason the recovery path is cheap. Register it in `routes/mod.rs` alongside the discover route.

### 4.4 Client preflight & layered error UX (fixes issue 2, timer-free)

**Two surfaces, each fed what suits it:**

1. **In-player overlay** (`has-media-error` / `.player-error-indicator`) — terse, player-appropriate copy ("Transcoding failed — the server couldn't start FFmpeg"), no navigation chrome.
2. **Global `error-panel` banner** (`state.error`) — the actionable layer: same failure rendered with an **[Open settings]** button that appears only when the structured error's `action == "open_settings"`.

**State plumbing.** Add to `AppState` (`types.ts:100`):

```ts
playbackError?: { code: string; message: string; action?: string };
```

Rendering in `renderMediaPlayerOverlay` (`playbackController.ts:324-327`) becomes dynamic, reading `state.playbackError` instead of the hardcoded string. `setPlayerError()` (currently `playbackController.ts:1053`) sets it.

**Preflight (timer-free).** In `startPlayback` (`playbackController.ts:610`), before opening the player on a doomed stream:
- If `state.capabilities.transcoding.ffmpeg.available === false` **and** the decision requires transcode → block; set `state.playbackError` and `state.error` from the `transcode_executable_missing` code.

**Stream-error detection (timer-free) — probe rejected.** An earlier draft proposed a `fetch()` probe of the stream URL before attaching `<video src>`. That is rejected: each `/sessions/<id>/stream` request spawns a fresh transcode (`media.rs:3619-3621`, child `kill_on_drop` at `transcode.rs:233`), so a probe + media-element request double-spawns ffmpeg on every successful playback (doubled startup latency and CPU), and any bytes read during the probe are buffered then discarded (memory + wasted work on the hot path).

Instead:
- **Primary detection — capabilities preflight (deterministic).** This phase produces exactly one spawn-time structured error, `ExecutableMissing`, which is precisely the signal `state.capabilities.transcoding.ffmpeg.available === false` already carries. So the preflight above catches the reported issue without touching the stream — no probe, no double spawn.
- **Best-effort recovery via a status read, not a re-spawn.** The `<video>` `error` handler (`playbackController.ts:1330-1333`) stays wired. When it fires, the client fetches a lightweight **session-status read** (a lookup of the per-session error seam in §4.3 — `ACTIVE_SESSION_ERRORS`, *not* a transcode spawn) to recover the structured `code`/`message`/`action` and drive both surfaces. Because it is a map lookup, it has none of the probe's costs. It only helps for the narrow case the preflight cannot see (e.g. a rare `Io` spawn error such as disk-full) and only when the browser actually fires the event — but it costs nothing on the success path.
- The cases a probe would genuinely help with (input errors, mid-stream failures) are exactly the **deferred lifecycle phase** (Appendix B), and there the correct architecture is a persistent transcode the stream *attaches* to, not a probe — a further reason not to lock in the probe pattern now.

## 5. Testing approach (matches + extends project conventions)

The project splits tests by kind: pure logic lives in **inline `#[cfg(test)] mod tests`** (e.g. `media.rs`, `metadata/providers/tvdb.rs`); **HTTP routes** are covered by integration tests in `crates/server/tests/test_web/routes/*.rs` using `create_test_client` + `make_request`.

- **`ffmpeg_resolve.rs` — inline unit tests** (highest value, fastest). Resolution precedence (absolute wins; bare-name falls back to well-known dirs; missing-everywhere returns `checked_paths`), staged with `tempfile::tempdir()` fake `ffmpeg`/`ffprobe` shims so tests are hermetic and require no real ffmpeg. This **improves on the current capabilities code, which has zero tests** despite being the failing component.
- **`transcode.rs` — inline unit tests (gap F).** Arg generation: a source path with spaces/brackets survives as a single argv element; `-c:v copy`/`-c:a copy` branches; fragmented-mp4 movflags. No transcode tests exist today; this closes that gap.
- **Real-ffmpeg smoke test, opt-in** behind `#[cfg(feature = "real-ffmpeg-tests")]` (or an env-var skip) so CI without ffmpeg still passes and anyone with ffmpeg locally can opt in.
- **Route integration test** — new file `crates/server/tests/test_web/routes/tools.rs` (mirroring `settings.rs`): discover endpoint returns `200` + a candidates array; `AdminGuard` rejects unauthenticated callers (matching the auth-status convention used in `test_auth.rs`). The session-status endpoint returns `200` with `error: null` (its inert value this phase) for a known session and `404` for an unknown one.
- **Frontend:** the project has **no client-web test harness** (no vitest/jest), so per project style client changes stay test-light (manual verification) in this scope. This is a deliberate, documented scoped decision — introducing a frontend test runner is out of scope.

## 6. Notes

### 6.1 UI conventions appendix (no design docs exist in-repo)

`docs/` has no UI/UX documentation. The conventions are implicit in code; the new components follow them rather than introducing a design system:

- **Error display — two existing surfaces:** (a) one global `state.error: string | undefined` rendered as `<section class="error-panel">` (`auth.ts:73`, used app-wide); (b) a player-local `has-media-error` → `.player-error-indicator` overlay (`playbackController.ts:324-327`, `style.css:2591-2609`). This spec unifies playback failures to drive **both**, each with copy suited to its context.
- **Settings forms:** labeled `<input>` rows + save button (`settingsView.ts:554-555`). No radio-list component exists today; the directory-radio list is a small new component, styled to match existing settings inputs, documented as new.
- **Realtime:** none in the codebase (only `setInterval` is the trailer UI tick at `playbackController.ts:997`); this spec introduces no new realtime transport.

### 6.2 Why not SSE (rejected for this scope)

SSE would mean a new transport, a server endpoint, reconnection logic, and a client subscription model — all for a path that is fundamentally *request → response* (one error per stream attempt). The structured JSON error body (§4.3) plus the deterministic capabilities preflight (§4.4) covers the same need using patterns the codebase already has. SSE earns its place only if realtime playback health (quality metrics, adaptive bitrate) becomes a goal — separate spec.

### 6.3 ffmpeg checks

- We haven't talked about transcoding codecs, but if the list of available ffmpeg binaries could also show, via some additional info dialog, or "show more" kinda behavior, the list of "available" codecs in each binary, that would be great, but we can consider a future improvement. But how hard would be to also include this info?
- Is there any risk that the current ffmpeg args, may not work with different ffmpeg versions? Should we have a compatibility check too in the future? While making the default template somehow customizable for admins, I kinda want to postpone this as much as possible, and if we really need we could have some compatibility check for that as well.

### 6.4 Setup wizard

Should we include in the current first start setup, an optional, collapsed, similar component to find, or manually provide, the ffmpeg binaries, but totally skippable? So eager admins do configure it in that screen, and other admins just set their user details and move on? I believe that screen may be reused for creating regular users too, so I'd be cautious about doing it there, but maybe it is a good idea, or in a follow up screen with a "Skip, I'll do it later" button? Let's get everything working first, but I think this is a good improvement for later.

## Appendix A — Files touched (planning reference)

Server:
- `crates/server/src/ffmpeg_resolve.rs` (new) — resolver + inline tests
- `crates/server/src/transcode.rs` — resolved-path spawn, `SpawnTranscodeError`, shared mapper, inline tests
- `crates/server/src/media.rs` — `detect_binary` delegates to resolver
- `crates/server/src/web/routes/media.rs` — discover route, session-status route, structured stream-error body, per-session error store (seam), route handler mapping
- `crates/server/src/web/routes/mod.rs` — register discover + session-status routes
- `crates/server/src/auth.rs` / wherever `AdminGuard` lives — no change, just used
- `crates/server/tests/test_web/routes/tools.rs` (new) — route integration tests

Client:
- `crates/client-web/src/api.ts` — `discoverTranscodingTools()`, `TranscodeErrorBody` type
- `crates/client-web/src/app/types.ts` — `playbackError` on `AppState`
- `crates/client-web/src/app/settingsView.ts` — detect button + directory radios
- `crates/client-web/src/app/playbackController.ts` — preflight, status-read recovery, dynamic overlay copy

## Appendix B — Deferred: Transcode lifecycle & mid-stream failure handling

The deferred items — (C-early) first-byte / early-exit stream validation, (C-mid) mid-stream watcher + client watchdog, and (E) source-path existence check — are captured in full as standalone documents so nothing is relearned:

- **Discovery:** `docs/superpowers/specs/2026-06-18-transcode-lifecycle-discovery.md` — current transcode lifecycle mechanics, failure modes, hard constraints (no naive timers; reuse shipped seams; no new realtime transport; cost stays off the success path), open questions, and what is already built for this phase.
- **Draft plan:** `docs/superpowers/specs/2026-06-18-transcode-lifecycle-plan.md` — proposed event-driven first-byte race (not a fixed timer), mid-stream watcher writing to the shipped `record_session_error` seam, bounded client watchdog with healthy-suppression, and (E) reconciled with the existing `missing_since` column.

This phase leaves the two inert seams those documents depend on: the per-session error store + `record_session_error()` writer, and the shared `map_transcode_error` + `TranscodeErrorBody { code, message, action }` (§4.3).
