# Transcode Lifecycle & Mid-Stream Failure Handling — Draft Plan

- **Date:** 2026-06-18
- **Status:** Draft plan (deferred phase). Not approved for implementation; to be reviewed and detailed when this phase is scheduled. Companion to `2026-06-18-transcode-lifecycle-discovery.md`.
- **Purpose:** A concrete proposed approach for the three deferred items, grounded in the discovery doc. Open questions there (§5) must be resolved before this becomes an approved plan.

## 0. Scope (three items)

- **(C-early)** Validate the stream before committing to 200.
- **(C-mid)** Detect ffmpeg dying after data has flowed and surface it to the player.
- **(E)** Pre-spawn source existence check (esp. for network/removable paths).

Non-goals: changing the client error surfaces (already shipped), changing the `TranscodeErrorBody` shape (already shipped), introducing SSE/WebSocket (rejected, discovery §4.3).

## 1. Architectural decision to make first: persistent vs per-request transcode

This is discovery open-question 7 and it shapes everything below.

**Option A — Stay per-request (smaller change).** Keep the current model: each `/stream` request spawns its own ffmpeg. Add a pre-commit validation step and a watcher, both per-request. Lower risk; preserves exact current teardown semantics.

**Option B — Persistent transcode keyed by `(session_id, start_ms, audio_index)` (larger change).** A long-lived ffmpeg process that stream requests attach to, eliminating the per-request re-spawn (and the double-spawn concern that killed the probe). Natural home for a single watcher. Higher risk; touches session lifecycle, replacement semantics, and concurrency (multiple readers of one stdout).

**Recommendation (tentative, to confirm in planning):** Start with **Option A**. It delivers the three scope items with the least surface area, reuses the shipped seams directly, and leaves Option B as a follow-on if re-spawn cost or reader-fan-out ever becomes a problem. The rest of this draft assumes Option A. The user affirmed there may be many users using the client in parallel, so per-request has it's benefits, but persistent keyed also allows the admin to kill some stalle transcoding spawn that for some reason is still running. Investigate further impacts unrelated to the stream-experience as another task too. And the user thinks "how do we handle transcoding caching?" has to be answered before deciding on anything other than option A.

## 2. (C-early) Pre-commit stream validation — event-driven, not a fixed timer

Replace "return stdout immediately" with "return stdout only once ffmpeg has proven it can produce output (or fail)".

### 2.1 Mechanism: a three-way race

On a successful spawn, before returning `SessionStream::Transcode`, race three futures:

1. **first-byte** — `stdout.read(&mut [u8; 1])` resolves on the first output byte.
2. **child-exit** — `child.wait()` resolves if ffmpeg exits before any byte.
3. **stderr-ready** — a line-buffered read on `stderr` resolves when ffmpeg writes a diagnostic line.

Outcomes:
- **first-byte wins** → success. Re-inject the read byte (push it back via a small `BufReader`/chain ahead of the returned stream) and return the stream as today.
- **child-exit wins (non-zero)** → read remaining stderr, map via `map_transcode_error` to a `BadInput`/`Io` `TranscodeErrorBody`, and return the structured error instead of 200. (No committed stream to abort.)
- **stderr-ready wins with a fatal-looking line** (e.g. "No such file", "Invalid data found", "could not open") → treat as likely failure: continue racing first-byte vs child-exit for a short, **generous and overridable** grace window before concluding. This is the *only* place a timeout appears, and it exists solely to avoid hanging on inputs that emit warnings then stall.

### 2.2 Why this satisfies "no naive timers"

The happy path resolves on the first-byte future — no timeout involved. A timeout appears only on the ambiguous stderr-but-no-byte path, is generous (default e.g. 10–15s to tolerate slow network mounts), and is configurable. This is the disciplined use of a bound the discovery doc (§4.1) calls for, not a blanket "wait 3s then guess".

### 2.3 Byte re-injection detail

The validation reads bytes from stdout; those bytes must still reach the client. Wrap stdout in a `BufReader<ChildStdout>`, peek via `fill_buf()` (non-consuming) rather than `read()`, and hand the `BufReader` to `ReaderStream`. `fill_buf` lets us see whether bytes are available without consuming them — so on success nothing is re-injected; it was never removed. Resolve this concretely in planning (confirm `tokio::io::AsyncBufReadExt::fill_buf` semantics against `ReaderStream`'s ownership model).

### 2.4 Failure mapping

`SpawnTranscodeError::BadInput { ffmpeg_stderr }` (already declared, inert in the shipped phase) becomes the variant produced here. `map_transcode_error` already maps `BadInput → 422` (status code to confirm). No new shape.

## 3. (C-mid) Mid-stream watcher + client watchdog

### 3.1 Server watcher

Once the stream is committed (first byte observed), spawn the existing-style background task but restructure it (discovery §2.1 fix):

- **Line-buffered stderr** instead of `read_to_end`, so diagnostics surface live and the "warnings but no bytes" case is detectable going forward.
- On **child exit (non-zero)** after data has flowed → call `record_session_error(session_id, ...)` (the shipped inert writer) with a `TranscodeErrorBody` produced by `map_transcode_error`. This populates `ACTIVE_SESSION_ERRORS`.
- Keep the post-mortem logging.

The watcher is per-request under Option A; under Option B it would be per-persistent-transcode.

### 3.2 Client watchdog (deliberate, bounded poll)

No new transport (discovery §4.3). A bounded `setInterval` against `GET /sessions/<id>/status`:

- **Cadence:** coarse (e.g. every 3–5s). Investigate exact value in planning (open question 3).
- **Suppress when healthy:** do not poll while the `<video>` is actively progressing — re-arm the poll only on `waiting`/`stalled`/`ended-unexpectedly`, or use `timeupdate`/`playing` to reset a "last progress" timestamp and poll only when that timestamp is stale beyond a threshold. Define the stall threshold for piped fragmented MP4 (open question 4) — distinguish natural between-fragment pauses from a dead stream.
- **On `status.error` non-null** → write `state.playbackError` from the body (reusing shipped plumbing) so the overlay + banner render with appropriate `action`.
- **Teardown:** clear the interval on player close and on session delete.

This keeps the success path cheap: the interval is suppressed while playback is healthy, and the status read is a map lookup, not a transcode spawn (discovery §4.4).

### 3.3 Interaction with the shipped `<video>` error handler

The shipped best-effort recovery (fetch status on `<video>` `error`) stays. The watchdog is the *active* detector for stalls; the error handler is the *reactive* detector for explicit media-element failures. Both funnel through `state.playbackError`.

## 4. (E) Pre-spawn source existence check

- Before spawning, validate the resolved source path is readable. On failure return **404** (not a confusing ffmpeg stderr path).
- **Reconcile with `missing_since`** (discovery open question 6): the backing-file row already has `missing_since` (`media.rs:3145`). Decide whether a per-play check also *sets* `missing_since` (so the UI/library can reflect it) or only returns 404. Prefer marking `missing_since` to avoid a parallel notion of "missing".
- Network/removable paths are the primary motivation; for local indexed files this is cheap insurance.
- Check the need to have an "hydration" method for network attached paths. And/or some addtional setting/checkbox in the library config of said path to allow features and improvements only for network paths.

## 5. Implementation phases (suggested sequencing)

1. **(C-early) first-byte race + byte re-injection + failure mapping.** Highest user-visible value (kills the "200 + dead stream" class). Validates the `fill_buf` approach.
2. **(C-mid server) watcher writes per-session errors.** Depends on the shipped seam; low risk.
3. **(C-mid client) bounded watchdog with healthy-suppression.** Depends on (2) and on resolving cadence/stall thresholds (open questions 3, 4).
4. **(E) source existence + `missing_since` reconciliation.** Independent; can land anytime.

Each phase is independently shippable and testable.

## 6. Testing approach (extends shipped conventions)

- **(C-early) inline tests in `transcode.rs` / a new module:** race behavior using a fake child whose stdout/stderr/exit are controlled by the test (shims, as in the shipped resolver tests). Assert: first-byte → stream returned; early exit → structured error, no stream; stderr-fatal-then-stall → structured error after grace.
- **(C-mid server) route test:** simulate a transcode that exits non-zero mid-stream; assert `GET /sessions/<id>/status` returns the error body; assert a clean session returns `null`.
- **(C-mid client) manual + (future) harness:** the project has no frontend test runner today; watchdog behavior is manual until one exists (call out as scoped).
- **(E) route test:** missing source path → 404 and `missing_since` set; present → unchanged.
- Real-ffmpeg smoke tests remain behind the opt-in flag from the shipped phase.

## 7. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Byte re-injection corrupts the stream | Use `fill_buf` (non-consuming peek) so no re-injection is needed; add a byte-exact test. |
| Grace timeout mistunes for slow mounts | Make it generous + configurable; race resolves on first byte for the common case. |
| Watchdog false-positives on between-fragment pauses | Define stall via child-exit observation + stale-progress threshold, not raw byte rate (open question 4). |
| Option B temptation creep | This plan commits to Option A; Option B is a separate future decision. |
| Parallel "missing" notions | Reconcile with `missing_since` in (E) rather than adding new state. |

## 8. Preconditions inherited from the shipped phase

This phase assumes the shipped spec landed: the `ACTIVE_SESSION_ERRORS` store + `record_session_error` writer, the `GET /sessions/<id>/status` reader, the shared `map_transcode_error` + `TranscodeErrorBody`, and the client `state.playbackError` + overlay/banner plumbing. Confirm these are present before starting; if any were descoped, update this plan.
