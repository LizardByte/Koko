# Routes Module Split — Plan

- **Status:** Plan for delegation (separate PR). Not implemented on the `feat/transcode-ffmpeg-resolution` branch.
- **Author context:** drafted during the transcode-fix work, after a first manual attempt at extracting modules proved too fragile for a single sitting (string-matching edits on a ~4,900-line file corrupted it once). This plan captures the target structure and the dependency map gathered then, so the delegate starts informed.
- **Scope:** split `crates/server/src/web/routes/media.rs` (~4,871 lines, 28 route handlers) into focused submodules. A separate, later effort should split `crates/server/src/media.rs` (~6,411 lines, the data layer) — that one needs its own design pass and is out of scope here.

## Why

`web/routes/media.rs` is a grab-bag: nearly every route in the app lives there, plus their response types and private helpers. Every PR touches it; merge conflicts are common; the file is hard to navigate. The handlers fall into clearly cohesive groups, so the split is mostly mechanical — the risk is in getting the shared-helper boundaries right.

## Current state (as measured)

- 28 `#[openapi]` route handlers + 83 lines of inline tests.
- Route registration in `crates/server/src/web/routes/mod.rs` via two macros: `api_routes()` (`openapi_get_routes![...]`) and `spa_routes()` (`routes![...]`). Every entry is `media::<handler>`.
- Private helpers shared across handler groups (see Dependency map below).
- Module-level statics for playback session state, transcode tasks, metadata refresh, catalog scan, system activities.

## Target structure

Proposed new modules under `crates/server/src/web/routes/`:

| Module | Responsibility | Routes (current names) |
|--------|----------------|------------------------|
| `system.rs` | Server capabilities, transcoding-tool discovery, session status, system activities, metadata providers/locales | `get_server_capabilities`, `discover_transcoding_tools`, `get_session_status`, `get_system_activities`, `get_metadata_providers`, `get_metadata_locales` |
| `libraries.rs` | Library config + catalog + inventory | `get_libraries`, `scan_library`, `delete_library_missing_items`, `refresh_library_metadata`, `get_library_inventory`, `add_library`¹, `remove_library`¹ |
| `items.rs` | Media item reads, metadata, people, search | `get_home`, `get_items`, `get_item`, `get_item_metadata`, `get_person`, `get_person_image`, `search_item_metadata`, `link_item_metadata`, `refresh_item_metadata`, `search_items` |
| `playback.rs` | Playback sessions, streams, progress, item-playback decisions | `create_session`, `delete_session`, `get_session_stream`, `get_item_playback`, `stream_item`, `update_item_progress` |
| `assets.rs` | Media asset serving (artwork, theme, subtitle) | `get_item_artwork`, `get_item_theme`, `get_item_subtitle` |
| `common.rs` | *(existing)* SPA index/asset routes — unchanged | `index`, `spa_asset` |
| `auth.rs`, `user.rs`, `settings.rs`, `dependencies.rs` | *(existing)* — unchanged | — |

¹ `add_library` / `remove_library` currently live in `settings.rs`; consider whether they belong with `libraries.rs` (semantic fit) or stay in `settings.rs` (current location). Recommend moving them to `libraries.rs` for cohesion, updating `settings.rs` and `mod.rs` accordingly.

`media.rs` is then deleted; `mod.rs` declares the new `pub mod` entries.

## Dependency map (the part that needs care)

These private helpers and shared state must be placed where all their callers can reach them without creating import cycles. Recommended homes:

**Shared response types** → keep in a new `routes/types.rs` (or in `common.rs`) if referenced by >1 module; otherwise move with their sole consumer:
- `ServerCapabilitiesResponse`, `BinaryProbe`, `ToolCandidate`, `ToolDiscoveryResponse`, `SessionStatusResponse`, `SessionTranscodeError` → `system.rs` (sole consumer is the system routes).
- `SessionStream`, `RangedFile`, `RangeHeader` → `playback.rs` (used by playback + assets streams).
- `ToolCandidate` etc. are system-only.

**Helpers** (defined line → callers):
- `open_ranged_file` (263) → callers: `get_session_stream` (3694), `stream_item` (3865). **Both are playback/assets.** → `playback.rs`.
- `content_type_for_path` (256) → caller: `open_ranged_file` (295). → moves with it to `playback.rs`.
- `stop_active_transcode` (303) → callers: `get_session_stream` (3651, 3687, 3757). → `playback.rs`.
- `replace_active_transcode` (313) → caller: `get_session_stream` (3782). → `playback.rs`.
- `record_session_error` (527) → caller: `get_session_stream` (3800). → `playback.rs`. (`get_session_status` in `system.rs` reads the same store — see "Session state" below.)
- `current_user_id` (2862) → callers: `update_item_progress` (3386), `create_session` (3419), `delete_session` (3499). All playback. → `playback.rs`. (If items/auth also need it later, promote to `common.rs`.)
- `probe_configured` (3053), `probe_in_dir` (3090) → sole caller: `discover_transcoding_tools`. → `system.rs`.

**Module-level statics** — the cross-module sharing point:
- `ACTIVE_PLAYBACK_SESSIONS`, `ACTIVE_TRANSCODE_TASKS`, `ACTIVE_SESSION_ERRORS` → these are **read by `system.rs` (`get_session_status`) and written by `playback.rs`**. Recommended: place them in `playback.rs` as `pub(crate)` with small accessors (`session_exists`, `session_error`), and have `system.rs` call those accessors. This keeps the writer owning the state and the reader using a narrow interface.
- `ACTIVE_SYSTEM_ACTIVITIES`, `ACTIVE_METADATA_REFRESH_*`, `ACTIVE_MANUAL_CATALOG_SCAN_RUNNING` → stay with their consumers (`system.rs` / `libraries.rs`).

**Imports** — every new module needs its own `use` block. The current `media.rs` has a large import list (~80 lines); each new module takes only the subset it uses. Watch for `Serialize`/`JsonSchema`/`openapi`/`Lazy`/`HashMap` which most modules need.

## Recommended execution order (each step independently shippable)

Each step is a separate commit; run `cargo test -p koko` + `cargo build -p koko` after each. The macros in `mod.rs` are updated per step.

1. **Extract `system.rs`** (capabilities, discover, status, activities, providers/locales). Depends on playback exposing `session_exists`/`session_error` — so do step 2's accessors first, or temporarily keep `get_session_status` in `media.rs` and move it in step 2. Lowest coupling, good first step.
2. **Extract `playback.rs`** (sessions, streams, progress, item-playback). Moves `SessionStream`/`RangedFile`/`RangeHeader`/`open_ranged_file`/`content_type_for_path`/`stop_active_transcode`/`replace_active_transcode`/`record_session_transcode`/`current_user_id` and the three `ACTIVE_*` statics (as `pub(crate)` + accessors). This is the largest, most intertwined step — do it carefully and alone.
3. **Extract `assets.rs`** (artwork, theme, subtitle). Depends on `playback.rs`'s `RangedFile`/`open_ranged_file` if subtitle/theme use ranged serving — verify and import as `super::playback::`.
4. **Extract `libraries.rs`** (library routes; optionally pull `add_library`/`remove_library` from `settings.rs`).
5. **Extract `items.rs`** (everything remaining). At this point `media.rs` is empty → delete it; remove from `mod.rs`.
6. **Final pass:** run clippy, fmt, full test suite; verify the openapi spec still lists all 28 routes.

## Anti-patterns to avoid

- **Don't reformat the whole tree in the same PR.** The repo is not `cargo fmt`-clean today (many files have pre-existing formatting drift); sweeping them muddies the diff. Run `cargo fmt` only on files this PR actually moves.
- **Don't change behavior.** This is a pure move refactor: no logic edits, no renamed routes, no API changes. The openapi spec and all tests must be byte-for-byte equivalent before/after.
- **Don't do it as one giant commit.** Each module extraction is reviewable on its own; a monolithic move is unreviewable and conflict-prone.
- **Don't invent new abstractions.** No traits, no trait objects, no "BaseRoute" helpers. Just moving functions/types between files. If a helper ends up shared, `pub(crate)` it; don't over-engineer.

## Verification

- `cargo test -p koko` — all 340+ tests pass unchanged.
- `cargo build -p koko` — no warnings introduced (watch for moved-but-unused imports).
- The generated openapi spec (if checked) lists the same 28 routes with the same paths/methods/schemas.
- `npx tsc --noEmit` in `crates/client-web` — the client uses `requestJson` against paths, not Rust module paths, so it should be unaffected; verify anyway.
- Manual smoke: settings, detect-ffmpeg, play an item (direct + transcode), metadata refresh.

## Follow-up: data layer split (`crates/server/src/media.rs`)

Separately, `media.rs` (~6,411 lines, ~52 public fns spanning library config, catalog sync, item queries, search, playback decisions, playback progress, metadata-link resolution, artwork/subtitle/theme resolution, transcoding capability) should be split by domain. This is higher risk (core app logic, cross-deps, less obvious boundaries) and deserves its own design doc + spec before execution. Suggested first investigation: build a call-graph of the 52 public fns to find natural seams, and check test coverage of each seam before cutting.
