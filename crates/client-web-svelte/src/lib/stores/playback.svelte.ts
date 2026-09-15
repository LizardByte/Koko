// Playback store — session lifecycle + player element state + actions.
// Replaces vanilla playbackController.ts (1385 lines of imperative DOM) with
// reactive $state that Svelte components bind to directly.
//
// Backend contracts followed exactly (no changes):
//   - Session lifecycle: createPlaybackSession on start, deletePlaybackSession on close
//   - Audio track switching: re-create session with new audio_stream_index
//   - Transcode seek: server-side start_ms offset for transcoded; client-side for direct-play
//   - Progress reporting: POST updatePlaybackProgress every 15s + on ended
//   - ClientProfile probe: getWebClientProfile canPlayType

import { pushState } from '$app/navigation';
import {
  createPlaybackSession,
  deletePlaybackSession,
  getSessionStreamUrl,
  getWebClientProfile,
  updatePlaybackProgress,
  type MediaItemDetail,
  type PlaybackSession,
} from '$lib/api';
import { noop } from '$lib/constants';
import { ui } from './ui.svelte';
import type { TrailerOption, ThemeSongSource } from '$lib/playerTypes';

// Escalating seek steps (seconds) — repeated presses within the window
// increase the step. Matches vanilla ESCALATING_SEEK_STEPS.
const ESCALATING_SEEK_STEPS = [10, 20, 30, 60, 120, 300] as const;

class PlaybackStore {
  // --- Session state ---
  session = $state<PlaybackSession | undefined>(undefined);
  item = $state<MediaItemDetail | undefined>(undefined);
  startMs = $state(0);
  activeAudioStreamIndex = $state<number | undefined>(undefined);

  // --- Overlay mode ---
  mode = $state<'media' | 'trailer' | undefined>(undefined);
  activeTrailer = $state<TrailerOption | undefined>(undefined);
  themeSongSource = $state<ThemeSongSource | undefined>(undefined);

  // --- Player element state (bound to <video>/<audio> via MediaPlayer) ---
  isPlaying = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  volume = $state(1);
  muted = $state(false);
  isLoading = $state(false);
  hasError = $state(false);
  isFullscreen = $state(false);
  isPictureInPicture = $state(false);
  controlsVisible = $state(true);

  // --- Resume prompt (Opportunity I) ---
  resumePrompt = $state<{ startMs: number; formatted: string } | undefined>(undefined);

  // --- Derived ---
  get isOpen(): boolean {
    return this.mode !== undefined;
  }

  get isAudio(): boolean {
    return this.item?.media_kind === 'audio';
  }

  /** The stream URL for the current session (with start offset + audio track). */
  get streamUrl(): string | undefined {
    if (!this.session) return undefined;
    const isAudioStreamOverride =
      this.activeAudioStreamIndex !== undefined && this.activeAudioStreamIndex > 0;
    const isRemuxingForAudio =
      isAudioStreamOverride && !this.session.decision.transcode_required;
    // Pass startMs for both transcoded and remuxed streams. For direct-play
    // (no transcode, no audio override), startMs is 0 (client-side seek handles it).
    const streamStartMs = (this.isTranscoding || isRemuxingForAudio) ? this.startMs : 0;
    return getSessionStreamUrl(
      this.session.session_id,
      streamStartMs,
      this.activeAudioStreamIndex,
    );
  }

  /** Whether the current session is transcoded (affects seek behavior). */
  get isTranscoding(): boolean {
    return (
      (this.session?.decision.transcode_required ?? false) ||
      (this.activeAudioStreamIndex !== undefined && this.activeAudioStreamIndex > 0)
    );
  }

  // --- Actions ---

  /**
   * Start media playback for an item. Creates a playback session, pushes a
   * history entry (Opportunity H — back-button closes the player), and checks
   * localStorage for a resume position (Opportunity I).
   */
  async start(item: MediaItemDetail, startMs: number): Promise<void> {
    // Opportunity I: check localStorage for a saved resume position.
    const savedMs = this.getResumePosition(item.id);
    if (savedMs !== undefined && startMs === 0) {
      // Only prompt if the saved position isn't near the start or end.
      const durationS = (item.duration_ms ?? 0) / 1000;
      const savedS = savedMs / 1000;
      if (savedS > 10 && (durationS === 0 || savedS < durationS - 10)) {
        const { formatMediaTime } = await import('$lib/format');
        this.resumePrompt = { startMs: savedMs, formatted: formatMediaTime(savedS) };
        // Store the intended start for when the user picks "Start fresh"
        this._pendingStart = { item, startMs };
        return;
      }
    }

    await this.beginPlayback(item, startMs);
  }

  /** Resume from the saved position (user clicked "Continue"). */
  async resumeFromPrompt(): Promise<void> {
    if (!this.resumePrompt || !this._pendingStart) return;
    const { item } = this._pendingStart;
    const resumeMs = this.resumePrompt.startMs;
    this.resumePrompt = undefined;
    this._pendingStart = undefined;
    await this.beginPlayback(item, resumeMs);
  }

  /** Dismiss the resume prompt and start fresh. */
  async dismissResumePrompt(): Promise<void> {
    if (!this._pendingStart) return;
    const { item, startMs } = this._pendingStart;
    this.resumePrompt = undefined;
    this._pendingStart = undefined;
    await this.beginPlayback(item, startMs);
  }

  private _pendingStart: { item: MediaItemDetail; startMs: number } | undefined = undefined;

  /** Internal: actually create the session + open the overlay. */
  private async beginPlayback(item: MediaItemDetail, startMs: number): Promise<void> {
    // Delete any prior session.
    if (this.session) {
      deletePlaybackSession(this.session.session_id).catch(noop);
    }

    this.item = item;
    this.startMs = Math.max(0, startMs);
    this.activeAudioStreamIndex = undefined;
    this.mode = 'media';
    this.hasError = false;
    this.isLoading = true;
    this.currentTime = 0;

    // Opportunity H: push a history entry so back-button closes the player.
    pushState('', { player: { itemId: item.id, startMs, kind: 'media' } });

    try {
      this.session = await createPlaybackSession({
        item_id: item.id,
        client_profile: getWebClientProfile(),
      });
      this.duration = (item.duration_ms ?? 0) / 1000;
    } catch (err) {
      this.hasError = true;
      this.isLoading = false;
      ui.setError(err instanceof Error ? err.message : 'Failed to start playback.');
    }
  }

  /**
   * Switch to a different audio track. Re-creates the session with the new
   * audio_stream_index (the server remuxes or transcodes). The stream URL
   * changes, so the <video> element reloads from the new start position.
   */
  async switchAudioTrack(streamIndex: number): Promise<void> {
    if (!this.item || !this.session) return;

    // Preserve current position for the new stream's start offset.
    this.activeAudioStreamIndex = streamIndex;
    this.startMs = Math.floor(this.currentTime * 1000);
    this.isLoading = true;
    this.hasError = false; // clear any stale error from the previous stream

    // Re-create the session with the new audio track.
    if (this.session) {
      deletePlaybackSession(this.session.session_id).catch(noop);
    }

    try {
      this.session = await createPlaybackSession({
        item_id: this.item.id,
        client_profile: getWebClientProfile(),
      });
    } catch (err) {
      this.hasError = true;
      this.isLoading = false;
      ui.setError(err instanceof Error ? err.message : 'Failed to switch audio track.');
    }
  }

  /** Close the media player + delete the session. */
  close(): void {
    if (this.session) {
      deletePlaybackSession(this.session.session_id).catch(noop);
    }
    this.session = undefined;
    this.item = undefined;
    this.mode = undefined;
    this.isPlaying = false;
    this.currentTime = 0;
    this.duration = 0;
    this.isLoading = false;
    this.hasError = false;
    this.activeAudioStreamIndex = undefined;

    // Navigate back (undo the pushState from beginPlayback).
    if (typeof history !== 'undefined' && history.state?.player) {
      history.back();
    }
  }

  // --- Trailer ---

  openTrailer(option: TrailerOption): void {
    this.activeTrailer = option;
    this.mode = 'trailer';
    pushState('', { player: { itemId: 0, startMs: 0, kind: 'trailer' } });
  }

  closeTrailer(): void {
    this.activeTrailer = undefined;
    if (this.mode === 'trailer') {
      this.mode = undefined;
    }
    if (typeof history !== 'undefined' && history.state?.player?.kind === 'trailer') {
      history.back();
    }
  }

  // --- Theme song ---

  playThemeSong(source: ThemeSongSource | undefined): void {
    this.themeSongSource = source;
  }

  stopThemeSong(): void {
    this.themeSongSource = undefined;
  }

  // --- Progress reporting (Opportunity D — called from MediaPlayer $effect) ---

  private _lastReportedSeconds = -1;

  /** Called by MediaPlayer on timeupdate. Reports every 15s boundary. */
  reportProgress(itemId: number, currentSeconds: number, durationMs: number | undefined): void {
    if (currentSeconds === this._lastReportedSeconds || currentSeconds % 15 !== 0) return;
    this._lastReportedSeconds = currentSeconds;

    const positionMs = Math.floor(currentSeconds * 1000);
    updatePlaybackProgress(itemId, {
      position_ms: positionMs,
      duration_ms: durationMs,
      completed: false,
    }).catch(noop);

    // Save to localStorage for resume prompt (Opportunity I).
    this.saveResumePosition(itemId, positionMs);
  }

  /** Called by MediaPlayer on ended. Reports completion. */
  reportCompleted(itemId: number, durationMs: number | undefined): void {
    updatePlaybackProgress(itemId, {
      position_ms: durationMs ?? 0,
      duration_ms: durationMs,
      completed: true,
    }).catch(noop);
    this.clearResumePosition(itemId);
  }

  // --- Escalating seek (pure logic, used by playerShortcuts action + buttons) ---

  private readonly _seekState = { lastDirection: 0, lastAt: 0, stepIndex: 0 };

  /**
   * Returns the seek step for the given direction, escalating if repeated
   * presses happen within the window. Used by the playerShortcuts action +
   * the seek buttons.
   */
  nextSeekStep(direction: number): number {
    const now = Date.now();
    if (
      direction !== 0 &&
      direction === this._seekState.lastDirection &&
      now - this._seekState.lastAt < 900
    ) {
      this._seekState.stepIndex = Math.min(ESCALATING_SEEK_STEPS.length - 1, this._seekState.stepIndex + 1);
    } else {
      this._seekState.stepIndex = 0;
    }
    this._seekState.lastDirection = direction;
    this._seekState.lastAt = now;
    return ESCALATING_SEEK_STEPS[this._seekState.stepIndex];
  }

  // --- localStorage resume positions (Opportunity I) ---

  private getResumePosition(itemId: number): number | undefined {
    try {
      const raw = localStorage.getItem(`koko:resume:${itemId}`);
      return raw ? Number(raw) : undefined;
    } catch {
      return undefined;
    }
  }

  private saveResumePosition(itemId: number, positionMs: number): void {
    try {
      localStorage.setItem(`koko:resume:${itemId}`, String(positionMs));
    } catch {
      // localStorage may be unavailable (private mode, quota).
    }
  }

  private clearResumePosition(itemId: number): void {
    try {
      localStorage.removeItem(`koko:resume:${itemId}`);
    } catch {
      // ignore
    }
  }
}

export const playback = new PlaybackStore();
