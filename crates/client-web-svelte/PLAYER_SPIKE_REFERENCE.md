# Player Spike — Architecture Reference & Opportunities

**Draft for discussion.** This document analyzes the vanilla `playbackController.ts` (1385 lines) and proposes a Svelte 5 port architecture, highlighting where we should follow vanilla with fidelity vs improve from the start.

---

## What the Vanilla Player Does

### Three playback surfaces
1. **HTML5 media player overlay** — `<video>` or `<audio>` element with session-based streaming. The main media player for movies/shows/episodes/music.
2. **YouTube trailer overlay** — YouTube IFrame Player API for trailers (YouTube URLs only). Separate overlay from the media player.
3. **YouTube theme song** — ambient background audio (non-overlay). Plays a theme song while browsing the item page.

### Session lifecycle (`startPlayback`)
```
createPlaybackSession({ item_id, client_profile }) → PlaybackSession
  → getSessionStreamUrl(session_id, startMs, audioStreamIndex) → URL
  → <video src={url}> (or <audio>)
  → on close: deletePlaybackSession(session_id)
```

### Key features (all in the media player overlay)
- **Dual audio tracks** — if the item has multiple audio tracks, switching creates a new session with a different `audio_stream_index`. The server remuxes (non-transcoded) or transcodes. A "Transcoding" badge appears during remux.
- **Subtitle tracks** — `<track>` elements injected for each subtitle, toggleable.
- **Seeking** — escalating seek (10→20→30→60→120→300s) for repeated keyboard/remote presses within 900ms. Regular ±10s for buttons.
- **Transcode vs direct-play** — transcoded streams can't seek client-side (the `<video>` starts from `startMs` server-side). Direct-play seeks via `player.currentTime`.
- **Progress reporting** — POSTs `updatePlaybackProgress` every 15 seconds of playback + on `ended`.
- **Picture-in-Picture** — `requestPictureInPicture()` on `<video>`.
- **Fullscreen** — `shell.requestFullscreen()`.
- **Auto-hide controls** — controls fade after 3.2s of inactivity while playing.
- **Keyboard shortcuts** — Space/k (play/pause), arrows (seek), m (mute), f (fullscreen), Escape (close).

### YouTube integration
- `loadYouTubeIframeApi()` — memoized script injection + `onYouTubeIframeAPIReady` callback.
- Trailer player — custom controls overlay (no YouTube native controls), volume/seek/play/pause/fullscreen.
- Theme song — hidden player, loops, pauses when player/trailer opens or navigating away.
- `extractYouTubeVideoId` — robust URL parsing (watch/embed/shorts/live/youtu.be/nocookie).

### Callbacks to the app
Vanilla injects `{ render, refreshData }` callbacks so the player can trigger re-renders (for audio-track switching, which recreates the session) and data refreshes (after progress updates).

---

## Proposed Svelte 5 Architecture

### Core principle: replace imperative DOM with reactive state

Vanilla is 1385 lines of imperative DOM manipulation: `document.querySelector` for every control, `addEventListener` for every interaction, `classList.toggle` for state changes, manual `render()` calls. Svelte eliminates ~80% of this via reactive state + template bindings.

### Component structure

```
src/lib/components/player/
├── PlayerOverlay.svelte        — top-level overlay: renders MediaPlayer or TrailerPlayer
├── MediaPlayer.svelte          — HTML5 video/audio player (the main player)
├── TrailerPlayer.svelte        — YouTube IFrame trailer overlay
├── ThemeSongPlayer.svelte      — ambient YouTube theme song (hidden)
├── PlayerControls.svelte       — shared controls bar (progress, transport, volume, fullscreen)
├── AudioTrackMenu.svelte       — audio track selection dropdown
└── YouTubeIframe.svelte        — reusable YouTube IFrame wrapper (used by trailer + theme)
```

### Store: `playback.svelte.ts`

```typescript
class PlaybackStore {
  // Session state
  session = $state<PlaybackSession | undefined>(undefined);
  item = $state<MediaItemDetail | undefined>(undefined);
  startMs = $state(0);
  isOpen = $state(false);
  activeAudioStreamIndex = $state<number | undefined>(undefined);

  // Player state (reactive — bound to the <video> element)
  isPlaying = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  volume = $state(1);
  muted = $state(false);
  isLoading = $state(false);
  hasError = $state(false);
  isFullscreen = $state(false);
  isPictureInPicture = $state(false);

  // Trailer state
  activeTrailer = $state<TrailerOption | undefined>(undefined);

  // Theme song state
  themeSongSource = $state<ThemeSongSource | undefined>(undefined);

  // Actions
  async start(item: MediaItemDetail, startMs: number) { ... }
  async switchAudioTrack(streamIndex: number) { ... }
  close() { ... }
  reportProgress() { ... }  // debounced, every 15s
}
```

### What gets dramatically simpler in Svelte

| Vanilla pattern | Svelte equivalent | Lines saved |
|---|---|---|
| `document.querySelector('#player-progress')` + manual `value` sync | `<input type="range" bind:value>` | ~40 |
| `addEventListener('timeupdate', updateTimeline)` | `$effect(() => { ... })` reacting to `currentTime` | ~30 |
| `classList.toggle('is-controls-visible')` + setTimeout hide | `$state(controlsVisible)` + reactive class | ~25 |
| `updateIconButton(button, 'pause', 'Pause')` + `createIcons()` | `<Icon name={isPlaying ? 'pause' : 'play'} />` | ~20 |
| `state.activeAudioStreamIndex = X; render(false)` (full re-render to swap audio) | `$state` mutation → Svelte updates only what changed | ~15 |
| `createEscalatingSeekHandler` closure | same logic, but cleaner (still needed) | 0 |
| `bindPlayerProgress()` + `bindTrailerPlayer()` (300+ lines of event binding) | Svelte `on:click` / `bind:` / `$effect` in template | ~250 |

**Estimated port size: ~500-600 lines** (vs 1385 vanilla) — the reactive model eliminates the DOM-query + event-binding + manual-render machinery.

---

## Opportunities for Improvement (discuss which to adopt)

### 1. Player as a route vs overlay ⭐
**Vanilla:** player is a full-screen overlay that sits on top of the current page (DOM-level, not a route).
**Improvement:** make the player a **SvelteKit route** (`/play/[itemId]`) so the URL reflects playback state, enabling:
- Browser back-button closes the player (not just Escape)
- Deep-linking to a specific playback position
- The player is a proper page, not a DOM overlay hack

**Trade-off:** vanilla's overlay approach lets the underlying page stay mounted (preserving scroll position, etc.). A route would unmount it. Could use a layout-level component that doesn't unmount children.

### 2. Native `<video controls>` vs custom controls ⭐
**Vanilla:** fully custom controls (progress bar, transport buttons, volume slider, etc.) — ~300 lines of markup + binding.
**Improvement:** use the browser's **native `controls` attribute** for the basic transport (play/pause/seek/volume/fullscreen), adding only custom UI for:
- Audio track selection (browsers don't expose this natively for remuxed streams)
- Subtitle track selection (browsers DO expose this natively via `<track>`)
- Title display + close button

**Trade-off:** vanilla's custom controls enable the escalating-seek, auto-hide, and consistent styling. Native controls are browser-dependent (Safari ≠ Chrome ≠ Firefox). Could be a **configurable** choice: `controls="native" | "custom"`.

### 3. Web Audio API for theme song ⭐
**Vanilla:** theme song uses a hidden YouTube IFrame player (heavy — loads the full YouTube JS API for ambient audio).
**Improvement:** if the theme song is a direct audio file (not YouTube), use a plain `<audio>` element (no YouTube API overhead). Only fall back to YouTube IFrame for YouTube-hosted theme songs.

**Trade-off:** vanilla already does this (`themeSongSourceFromUrl` checks for YouTube vs audio URL). The improvement is to make the `<audio>` path the primary and YouTube the fallback, not the other way around.

### 4. Reactive progress reporting
**Vanilla:** `setInterval`-like pattern via `timeupdate` event + `lastSentSeconds` tracking.
**Improvement:** a `$effect` that watches `currentTime` and reports when it crosses a 15-second boundary. Cleaner, no manual `lastSentSeconds` bookkeeping.

### 5. Keyboard shortcuts via Svelte actions
**Vanilla:** `shell.addEventListener('keydown', ...)` with manual key matching.
**Improvement:** a `use:playerShortcuts` Svelte action that encapsulates the keyboard map. Reusable, testable, declarative.

### 6. Cleanup via `$effect` return
**Vanilla:** manual `destroyTrailerYouTubePlayer()`, `clearTrailerProgressHandle()`, `deletePlaybackSession()` called from multiple sites.
**Improvement:** `$effect` with a cleanup function that runs when the player component unmounts — Svelte handles teardown automatically. No manual destroy calls.

### 7. Player state in URL search params
**Improvement:** `/items/101?play=true&t=1260000` — the play state is a query param, not separate state. Refreshing the page resumes playback. Shareable playback links.

### 8. Accessibility improvements
**Vanilla:** custom controls have basic ARIA labels but no screen-reader announcements for state changes.
**Improvement:** `aria-live` regions for play/pause/error states. `<input type="range">` with proper `aria-valuenow`/`aria-valuemin`/`aria-valuemax`.

---

## Fidelity Decisions to Make

For each, decide: **follow vanilla** (faithful port) or **improve** (new behavior).

| Area | Follow vanilla | Improve |
|---|---|---|
| Overlay vs route | Full-screen DOM overlay | SvelteKit route `/play/[id]` |
| Custom vs native controls | Custom (styled, escalating-seek) | Native + custom for audio tracks |
| YouTube integration | IFrame API (memoized) | Same (no better option exists) |
| Theme song | YouTube IFrame primary | `<audio>` primary, YouTube fallback |
| Session lifecycle | create/delete per play | Same (backend contract) |
| Audio track switching | Re-create session with new index | Same (backend requires it) |
| Transcode seek | Server-side start offset | Same (backend contract) |
| Progress reporting | POST every 15s + on ended | Same (or reactive $effect) |
| Escalating seek | [10,20,30,60,120,300] steps | Same (good UX, worth keeping) |
| Controls auto-hide | 3.2s timeout | Same (or configurable) |
| Keyboard shortcuts | Space/k/arrows/m/f/Esc | Same + maybe arrow-up/down for volume |

---

## Recommended Approach

1. **Follow vanilla for backend contracts** — session lifecycle, audio track switching, transcode seek, progress reporting. These match the Rust server's API; changing them would break the contract.

2. **Improve the frontend architecture** — reactive state instead of imperative DOM, `$effect` cleanup instead of manual destroy, Svelte actions for keyboard shortcuts. This is where Svelte shines and where we cut ~800 lines.

3. **Keep custom controls** (not native) — the escalating-seek, auto-hide, and consistent dark styling are worth the effort. But implement them reactively, not imperatively.

4. **Keep the overlay approach** (not a route) — the underlying page staying mounted is important for UX (scroll position, player toggling). A SvelteKit layout-level component achieves this without the URL complexity.

5. **Audio-first theme song** — if the theme song URL is a direct file, use `<audio>`. Only use YouTube IFrame for YouTube URLs. This is what vanilla does, just make it cleaner.

6. **YouTube helper** — port `youtube.ts` (URL extraction + IFrame API loader) as-is. It's clean, well-tested, and has no Svelte-specific concerns.

---

## Execution Roadmap (proposed)

### Phase A: Foundation (~200 lines)
- `playback.svelte.ts` store (session state + player state + actions)
- Port `youtube.ts` (URL extraction + IFrame loader) — as-is
- Port `playbackProgress.ts` — already done
- Port `mediaTargets.ts` + `mediaExtras.ts` — small helpers

### Phase B: Media Player (~250 lines)
- `MediaPlayer.svelte` — `<video>`/`<audio>` element + `PlayerControls.svelte`
- Session lifecycle (start/close/switch-audio-track)
- Progress reporting (reactive $effect)
- Escalating seek
- Controls auto-hide
- Keyboard shortcuts (Svelte action)

### Phase C: YouTube Overlays (~150 lines)
- `YouTubeIframe.svelte` — reusable wrapper
- `TrailerPlayer.svelte` — trailer overlay using YouTubeIframe + PlayerControls
- `ThemeSongPlayer.svelte` — ambient theme song

### Phase D: Integration (~50 lines)
- Wire HeroActions play/playTarget/playTrailer/playThemeSong → playback store
- Wire SectionExtras play → playback store
- Wire PlayerOverlay into +layout.svelte (rendered above page content when isOpen)
- Stories + CSS + verification

### Minor gaps (done in parallel)
- Never-scanned library polling (add to activities store)
- Standalone stories for LoginScreen/WelcomeScreen/AuthShell

**Total estimated: ~650 lines** (vs 1385 vanilla) — the reactive model cuts ~50%.
