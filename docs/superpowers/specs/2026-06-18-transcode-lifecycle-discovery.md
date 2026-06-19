# Transcode Lifecycle & Mid-Stream Failure Handling — Discovery

- **Date:** 2026-06-18
- **Status:** Discovery (deferred phase). Not implemented. Companion to `2026-06-18-transcode-ffmpeg-resolution-and-player-errors-design.md`.
- **Purpose:** Capture everything discovered about the transcode *lifecycle* so the deferred phase starts from complete knowledge and relearns nothing.

## 1. What this phase is about

The shipped spec (`...-player-errors-design.md`) fixes **issue 1** (ffmpeg not resolvable) and the **spawn-time + preflight** portion of issue 2, with no timers. It deliberately leaves three things to a later, timer-containing phase:

- **(C-early)** first-byte / early-exit validation of a stream before committing to a 200.
- **(C-mid)** mid-stream watcher (ffmpeg dies after data has flowed) + client watchdog.
- **(E)** source-path existence check for paths that vanish between scan and play.

This document records the current code mechanics, the constraints, and the open questions. The companion draft plan (`2026-06-18-transcode-lifecycle-plan.md`) proposes an approach.

## 2. Current transcode lifecycle (as it exists today)

Entry point: `get_session_stream` (`crates/server/src/web/routes/media.rs:3531`).

1. The handler resolves the session, builds a `TranscodeSpec`, and calls `stop_active_transcode(&session_id)` then `spawn_transcode_stdout(...)` (`media.rs:3619-3621`).
2. `spawn_transcode_stdout` (`crates/server/src/transcode.rs:214-237`) configures `stdin(null)`, `stdout(piped)`, `stderr(piped)`, `kill_on_drop(true)`, and spawns the child. It returns the `Child` immediately.
3. Back in the handler (`media.rs:3622-3649`): `child.stdout` is taken and returned inside `SessionStream::Transcode`. `child.stderr` is taken and handed to a **background `tokio::spawn` task** that does, in order:
   - `stderr.read_to_end(&mut stderr_text).await` — this resolves **only at EOF**, i.e. when ffmpeg closes stderr, i.e. when ffmpeg exits.
   - `child.wait().await` — reaps the process.
   - Logs the captured stderr and non-zero exit status.
4. The HTTP response returns `200` with `Content-Type: video/mp4` and a streamed body wrapping `ChildStdout` via `ReaderStream::one` (`media.rs:178-186`).

### 2.1 Why this is fragile

- **The 200 is returned before ffmpeg has produced any data.** If ffmpeg fails to open the input (truly missing file, unsupported codec, broken `-ss` seek, corrupt header), the route has already committed to 200 and the client receives a stalled or immediately-truncated stream. The background task logs the failure *afterward* — too late for the client.
- **stderr is buffered until exit.** `read_to_end` means no streaming stderr; useful diagnostics are only seen post-mortem.
- **No mid-stream signal to the client.** If ffmpeg dies after sending some data (disk error, network blip on a remote source, OOM kill), the client's stream just ends or stalls. Nothing writes a per-session error; nothing tells the player.
- **`stop_active_transcode` aborts the prior handle** (`media.rs:317-325`). An abort drops the task; with `kill_on_drop`, the prior ffmpeg is killed. This is correct for replacement, but means there is no notion of a *persistent* transcode a new request can attach to — each stream request re-spawns. (Relevant to why the rejected probe idea double-spawned; see the design doc §4.4.)

## 3. Failure modes the lifecycle phase must cover

| Mode | When | Current behavior | Desired behavior |
|------|------|------------------|------------------|
| Input open failure (missing/locked file, bad codec, seek error) | before first byte | 200 + dead stream; logged post-mortem | structured error before committing (C-early) |
| Early exit (ffmpeg starts, emits stderr, dies within seconds) | before/around first byte | 200 + truncated stream | structured error before committing (C-early) |
| Mid-stream death (disk/net/OOM after data flowed) | after first byte | truncated/stalled stream; no client signal | watcher writes per-session error; client watchdog surfaces it (C-mid) |
| Source vanished since scan (esp. network/removable paths) | at spawn/input-open | confusing ffmpeg stderr | clean 404 from a pre-spawn check (E) |

## 4. Constraints discovered now (must hold for the lifecycle phase)

1. **No naive timers.** A fixed `tokio::time::sleep(3s)` first-byte check is a bug magnet (too short for slow remote mounts; too long for snappy UX). Any bound must be event-driven first, with generous and overridable timeouts only where a bound is genuinely required.
2. **Reuse the shipped seams, don't invent new shapes.**
   - The **per-session error store** `ACTIVE_SESSION_ERRORS` + `record_session_error()` (design doc §4.3) is the writer the watcher uses.
   - The **shared `map_transcode_error`** + `TranscodeErrorBody { code, message, action }` is the single client-facing shape — mid-stream errors must map through it, not a second representation.
   - The **session-status read** (`GET /api/v1/sessions/<id>/status`, design doc §4.3) is the client's lookup; it returns `{ error: null }` today and will carry the watcher's error once populated.
3. **No new realtime transport.** The codebase has no SSE/WebSocket (only a `setInterval` for the trailer UI tick, `playbackController.ts:997`). The watchdog must be a deliberate, bounded poll, not a persistent socket. SSE was evaluated and rejected for the shipped phase; revisit only if realtime playback *health metrics* become a goal, not just failure signaling.
4. **Cost stays off the success path.** The whole reason the shipped phase rejected a probe was avoiding doubled ffmpeg startup and wasted buffered bytes on every successful play (each `/stream` request spawns fresh). The lifecycle phase must preserve that: a healthy stream must incur no extra spawns and no buffering tax.
5. **`kill_on_drop` replacement semantics must be preserved.** `stop_active_transcode` + `kill_on_drop` is how a new stream request replaces an old one. Any persistent-transcode model must keep correct teardown on session delete/replace.
6. **Client surfaces are already defined.** The in-player overlay (`has-media-error`, terse) and the global `error-panel` banner (actionable, conditional `[Open settings]` button on `action == "open_settings"`) are shipped and must be reused as-is — mid-stream failures feed the same `state.playbackError { code, message, action }`.

## 5. Open questions (to resolve during lifecycle planning, not now)

These are recorded so they are not lost; they need investigation/decisions in that phase.

1. **First-byte strategy.** Pure race (first-stdout-chunk Future vs child-exit Future vs stderr-ready signal), or first-byte race **plus** a generous stderr-wait timeout for inputs that emit warnings before producing bytes? What is "generous" — and should it be configurable (per-library, for slow network mounts)?
2. **Streaming stderr.** Switch from `read_to_end` to a line-buffered reader so diagnostics surface live and so "ffmpeg is emitting warnings but no bytes yet" is detectable without waiting for exit. Tradeoff: more moving parts in the background task.
3. **Watchdog cadence & suppression.** What interval, and how to suppress polling while healthy? Candidates: suppress on `<video>` `playing`/`timeupdate`; poll only when the element is stalled/buffering beyond a threshold. What threshold, and does it false-positive on legitimately slow networks?
4. **Stall definition for piped fragmented MP4.** Fragmented MP4 over a pipe can have natural pauses between fragments; the watchdog must distinguish "between fragments" from "dead". Needs a defined notion (bytes-per-window, fragment cadence, or just child-exit observation).
5. **Status endpoint vs 500-on-reconnect.** Is the `GET /sessions/<id>/status` read sufficient for the watchdog, or does the stream endpoint itself need to return the structured error on a *reconnect* attempt (e.g. after the player retries the src)?
6. **(E) scope.** Should source-existence be only a per-play 404, or also drive a server-side "missing since scan" background job (mark `missing_since`, surface in the UI)? The shipped code already has a `missing_since` column on backing files (`media.rs:3145`) — the lifecycle phase should reconcile per-play checks with that existing mechanism rather than duplicate it.
7. **Persistent vs per-request transcode.** Should a stream request attach to a long-lived transcode keyed by `(session_id, start_ms, audio_index)` instead of spawning fresh each time? This would eliminate the double-spawn concern entirely and is the natural home for a watcher — but it is a larger change. Decide whether the lifecycle phase takes it on or stays per-request.
8. **Tooling.** Would Rayon and/or https://github.com/tqwewe/kameo improve our handling of lifecycle, in or out, of transcoding? Where? How? Should we take it as something to plan now, or later? Do the project currently has parallel libs or actor libs besides what tokio and axum does?

## 6. What is already built for this phase (no rework needed)

- Per-session error store + `record_session_error()` writer — inert, ready to be called.
- `GET /sessions/<id>/status` reader — live, returns `{ error: null }` until the watcher writes.
- Shared `map_transcode_error` + `TranscodeErrorBody { code, message, action }`.
- Client `state.playbackError` + dynamic overlay/banner rendering + `[Open settings]` on `action`.
- Capabilities preflight + (now-rejected) probe rationale — so the lifecycle phase knows what was *not* done and why.

## 7. Related code references

- Stream handler: `crates/server/src/web/routes/media.rs:3531` (`get_session_stream`), spawn/error mapping at `:3619-3656`.
- Spawn + background task: `crates/server/src/transcode.rs:214-237`; the task body at `media.rs:3626-3643`.
- Replacement/teardown: `stop_active_transcode` / `replace_active_transcode` (`media.rs:317-338`).
- Error surfaces: `crates/client-web/src/app/playbackController.ts:1053` (`setPlayerError`), `:1330-1333` (`<video>` error handler), overlay/banner markup at `:317-360`.
- Existing `missing_since` mechanism: `crates/server/src/media.rs:3145` (relevant to open question 6).
