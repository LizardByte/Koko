# Koko roadmap

This file tracks the staged path from the current proof-of-concept into a complete self-hosted media platform.

## Product direction

Koko should grow as a single-repo Rust media platform with a strong shared core:

- Rust-first server and shared contracts
- FFmpeg-backed media inspection, transcoding, and packaging
- External-FFmpeg-first licensing posture, with an abstraction layer that keeps future embedded-library support possible if licensing allows
- TMDB-first metadata for movies and TV, with a provider model that keeps additional sources pluggable and user-selectable over time
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

Planned top-level crate layout:

- `crates/server`: Rust server, APIs, jobs, scanner, transcoding, auth, persistence
- `crates/common`: shared Rust domain models, API contracts, playback profiles, and utilities
- `crates/client-web`: browser UI and shared web assets
- `crates/client-desktop`: desktop shell and native integrations
- `crates/client-android`: Android and Android TV client
- `crates/client-ios`: iOS and Apple TV client
- `crates/client-roku`: Roku client if practical in-repo
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
- [ ] Add media item, collection, and search APIs
  - [x] Add media item detail and search API baseline
  - [x] Add metadata provider and item metadata status API baseline
- [ ] Add stream manifest and direct-play decision APIs
- [ ] Add FFmpeg transcoding sessions and job management
- [ ] Add background task coordination, progress, and cancellation
- [ ] Add API docs and stable contract review for browser client work

### Stage 2: Browser client

Status: In progress

Goal: ship the first real user-facing client.

Design target: a Kodi/Plex-inspired browsing experience with shelves, hero areas, poster art, and a media-first detail layout.

Checklist:

- [x] Create `crates/client-web`
- [ ] Implement login and session handling
- [ ] Implement library browsing and search
  - [x] Add Kodi/Plex-inspired shelf and poster card baseline
- [ ] Implement item details, artwork, and playback UI
  - [x] Add metadata provider status and linked metadata detail baseline
- [ ] Implement admin pages for libraries, users, and transcoding settings
- [ ] Add quality selection and playback error reporting

### Stage 3: Desktop app for Windows, Linux, and macOS

Status: Planned

Goal: deliver a packaged desktop experience by reusing browser-client functionality where possible.

Checklist:

- [ ] Define desktop shell architecture
- [ ] Reuse browser UI where practical
- [ ] Add native windowing, tray, deep links, and auto-start support
- [ ] Add packaging and signing workflows per OS

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
- [ ] Observability, metrics, tracing, and structured diagnostics
- [ ] Performance baselines for scan, metadata, and transcoding workflows
- [ ] Internationalization and accessibility
- [ ] CI, release automation, packaging, and update channels
- [ ] Dependency license and supply-chain review
- [ ] User documentation and deployment guides

## Metadata strategy

- TheMovieDB is the first planned online metadata provider for movies and TV.
- The metadata system should stay provider-agnostic so future sources for music, books, photos, and local sidecar metadata can be added without redesigning the core media catalog.
- Users should eventually be able to enable providers, set priority order, and choose which providers apply to each library type.

## FFmpeg strategy

- The current implementation path assumes external `ffmpeg` and `ffprobe` executables by default.
- This keeps the licensing path clearer for a source-available distribution model while still letting Koko use FFmpeg capabilities.
- The server architecture should keep a clean transcoding abstraction so embedded FFmpeg libraries remain a future option if licensing and distribution requirements are compatible.

## Current sprint

The current sprint is focused on the first shippable Stage 1 slice:

- media-library configuration model
- library discovery summaries
- FFmpeg capability detection
- versioned discovery endpoints for future clients
- persistent media-library catalog and file inventory baseline
- incremental rescans with stable media item IDs
- ffprobe-backed metadata persistence for audio and video files
- metadata-provider registry and item metadata link baseline
- browser-oriented item, detail, and search APIs
- initial `crates/client-web` scaffold consuming Stage 1 APIs
- Kodi/Plex-inspired poster and metadata detail baseline in `crates/client-web`

## Exit criteria for Stage 1

Stage 1 is complete when:

- the server can scan configured libraries and persist results
- metadata extraction is reliable enough to drive a browser UI
- direct play versus transcode decisions are available through stable APIs
- FFmpeg-backed playback sessions can be started, monitored, and stopped
- the browser client can browse and play media using supported server APIs
