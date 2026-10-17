# Koko roadmap

This file tracks the staged path from the current proof-of-concept into a complete self-hosted media platform.

## Product direction

Koko should grow as a single-repo Rust media platform with a strong shared core:

- Rust-first server and shared contracts
- FFmpeg-backed media inspection, transcoding, and packaging
- Shared build-deps FFmpeg executables, with an abstraction layer that keeps future embedded-library support possible
- TMDB and TheTVDB metadata for movies and TV, with per-library provider priorities and pluggable primary and secondary sources
- Browser client first
- Kodi/Plex-inspired browse and playback UX
- Desktop packaging next for Windows, Linux, and macOS
- Mobile and TV clients after the server and browser APIs stabilize
- Roku and Xbox after the streaming formats, remote UX, and compatibility story are mature

## Client priority order

1. Web browser
2. Windows, Linux, and macOS
3. Android, including Android TV
4. iOS, including Apple TV
5. Roku
6. Xbox One and Xbox Series

## Repo direction

The repo should remain the main home for the server and as many clients as practical.

Current crate layout and planned additions:

- `crates/server`: Rust server, APIs, jobs, scanner, transcoding, auth, persistence
- `crates/common`: shared Rust domain models, API contracts, playback profiles, and utilities
- `crates/client-web`: browser UI and shared web assets
- `crates/client-desktop`: desktop shell and native integrations
- `crates/client-android`: Android and Android TV client
- `crates/client-ios`: iOS and Apple TV client
- `crates/client-roku`: planned Roku client if practical in-repo
- `crates/client-xbox`: Xbox-focused client shell or shared console client assets if feasible

## Delivery stages

### Stage 1: Server core and browser-first API foundation

Status: In progress

Goal: make the server useful enough that a browser client can browse libraries, inspect media, authenticate, and start playback without redesigning the backend.

Checklist:

- [x] Create a roadmap and tracking file
- [x] Add media library configuration and discovery foundation
- [x] Add FFmpeg and ffprobe capability detection foundation
- [x] Add versioned server/media discovery endpoints for future clients
- [x] Add persistent media-library data model and migrations
- [x] Add filesystem scanner jobs and incremental rescans
- [ ] Add metadata extraction and artwork generation
  - [x] Add ffprobe-backed metadata extraction baseline
  - [x] Add pluggable metadata-provider registry and persistence baseline
  - [x] Add TMDB and TheTVDB primary providers, plus ThemerrDB and TrailerDB secondary providers
  - [x] Add per-library provider ordering, multiple metadata languages, manual matching, immediate unlink/rematch clearing, and provider-response cache controls
  - [x] Validate downloaded artwork, preserve working images on download failures, and reject obsolete metadata database writes
  - [x] Retain shared URL-addressed artwork so overlapping requests cannot delete newer covers
  - [ ] Generate optional video seek-preview thumbnails
- [x] Add media item, collection, and search APIs
  - [x] Add media item detail and search API baseline
  - [x] Add metadata provider and item metadata status API baseline
- [x] Add stream manifest and direct-play decision API baseline
- [x] Add FFmpeg transcoding sessions with start and stop management
- [ ] Add background task coordination, progress, and cancellation
  - [x] Add scheduled task windows, catalog scans, metadata refresh queues, and visible item/library progress
  - [ ] Include automatic show descendants in refresh progress and failures
  - [x] Recover all pending metadata translations in one pass
  - [ ] Add user-controlled cancellation for background work
- [ ] Add API docs and stable contract review for browser client work

### Stage 2: Browser client

Status: In progress

Goal: ship the first real user-facing client.

Design target: a Kodi/Plex-inspired browsing experience with shelves, hero areas, poster art, and a media-first detail layout.

Checklist:

- [x] Create `crates/client-web`
- [x] Implement login and session handling
- [x] Implement library browsing and search
  - [x] Add Kodi/Plex-inspired shelf and poster card baseline
- [x] Implement item details, artwork, and playback UI baseline
  - [x] Add metadata provider status and linked metadata detail baseline
- [x] Implement admin pages for libraries, users, providers, scheduled tasks, logs, and transcoding settings
- [ ] Add quality selection and playback error reporting
- [x] Add automated web component, rendering, permission, and navigation tests
- [x] Ignore superseded metadata polls after unlinking, rematching, or starting a newer refresh
- [ ] Add browser end-to-end coverage for metadata management and playback

### Stage 3: Desktop app for Windows, Linux, and macOS

Status: In progress (packaged server and tray baseline; dedicated desktop client remains planned)

Goal: deliver a packaged desktop experience by reusing browser-client functionality where possible.

Checklist:

- [ ] Define desktop shell architecture
- [ ] Reuse browser UI where practical
- [ ] Add native windowing, tray, deep links, and auto-start support
  - [x] Add server tray integration and opening the browser client
- [x] Add packaging and signing workflows per OS
  - [x] Add Windows installer, macOS DMG, and Linux Flatpak build workflows
  - [x] Add Windows signing and macOS code-signing and notarization support
- [ ] Validate signed package installation and update behavior on supported operating systems

### Stage 4: Android and Android TV

Status: Planned

Goal: mobile and TV playback with touch and remote-friendly UX.

Checklist:

- [ ] Define shared playback and authentication contracts in `crates/common`
- [ ] Build Android phone and tablet UX
- [ ] Build Android TV ten-foot UX
- [ ] Add offline sync and mobile network-aware playback policy

### Stage 5: iOS and Apple TV

Status: Planned

Goal: parity with Android while respecting Apple platform constraints.

Checklist:

- [ ] Implement iOS playback and navigation
- [ ] Implement tvOS experience
- [ ] Add platform-safe packaging, signing, and distribution flow

### Stage 6: Roku

Status: Planned

Goal: focused living-room experience once the server and stream formats are stable.

Checklist:

- [ ] Confirm in-repo feasibility and toolchain approach
- [ ] Implement remote-first browse and playback flows
- [ ] Validate supported codecs, subtitle handling, and fallback transcoding

### Stage 7: Xbox One and Xbox Series

Status: Planned

Goal: deliver console playback after the ten-foot UX and streaming stack are mature.

Checklist:

- [ ] Confirm platform delivery approach and repo fit
- [ ] Implement controller-friendly browse and playback flows
- [ ] Validate console-specific playback constraints and packaging requirements

## Cross-cutting workstreams

These run across multiple stages:

- [ ] Security hardening, secrets handling, and audit logging
  - [x] Enforce administrator and per-library access controls, and store provider credentials in the operating-system secret store
- [ ] Observability, metrics, tracing, and structured diagnostics
- [ ] Performance baselines for scan, metadata, and transcoding workflows
- [ ] Internationalization and accessibility
- [ ] CI, release automation, packaging, and update channels
  - [x] Run server and web tests, web builds, and platform package builds in CI
  - [ ] Validate hosted checks and platform packages for the metadata reliability changes
- [ ] Dependency license and supply-chain review
- [ ] User documentation and deployment guides

## Metadata strategy

- TheMovieDB and TheTVDB are implemented primary metadata providers for movies and TV.
- ThemerrDB extends movie, show, and collection metadata with themes; TrailerDB adds movie and show extras through supported TMDB references.
- The metadata system should stay provider-agnostic so future sources for music, books, photos, and local sidecar metadata can be added without redesigning the core media catalog.
- Users can enable providers, set their priority order per library, and select library metadata languages. Local NFO, music, and book providers remain planned.
- Provider responses use endpoint-specific models, and refresh jobs bypass response caches while keeping the normal cache expiry and manual cache-clear controls.
- Unlinking a movie or show immediately clears its provider metadata and any show descendants, and leaves it unlinked until an explicit rematch. Rematching clears the old metadata before fetching replacement data in the background.

## FFmpeg strategy

- Packaged builds include shared build-deps `ffmpeg` and `ffprobe` executables; custom paths remain supported.
- This keeps the licensing path clearer for a source-available distribution model while still letting Koko use FFmpeg capabilities.
- The server architecture should keep a clean transcoding abstraction so embedded FFmpeg libraries remain a future option if licensing and distribution requirements are compatible.

## Current sprint

The current focus is reliability of the implemented Stage 1 server and Stage 2 browser workflows:

- keep TMDB, TheTVDB, and secondary-provider decoding aligned with their endpoint contracts
- make movie, show, season, and episode refreshes observable and recoverable
- prevent obsolete downloads and browser polls from undoing an unlink, rematch, or newer refresh
- finish automatic descendant progress and recovery of all pending metadata locales
- preserve existing artwork and secondary metadata across transient provider failures
- validate these workflows with regression tests, browser end-to-end coverage, and hosted platform checks before release

## Exit criteria for Stage 1

Stage 1 is complete when:

- the server can scan configured libraries and persist results
- metadata extraction is reliable enough to drive a browser UI
- direct play versus transcode decisions are available through stable APIs
- FFmpeg-backed playback sessions can be started, monitored, and stopped
- the browser client can browse and play media using supported server APIs
