/** Defines playback quality targets and safe recovery actions for browser media failures. */

import { ApiError, type ClientProfile, type PlaybackDecision, type PlaybackSessionStatus } from '../api';
import { escapeHtml } from './format';

/** A user-selected upper bound; Original retains automatic codec compatibility. */
export type PlaybackQuality = 'original' | '4k' | '1080p' | '720p' | '480p';

/** Recovery choices available for a failed playback attempt. */
export interface PlaybackProblem {
  message: string;
  retry: boolean;
  compatible?: boolean;
  signIn?: boolean;
}

/** Fixed quality targets expressed as maximum dimensions and total bitrate in kbps. */
export const PLAYBACK_QUALITIES = [
  { id: 'original', label: 'Original quality', width: 0, height: 0, bitrate: 0 },
  { id: '4k', label: '4K · 25 Mbps', width: 3840, height: 2160, bitrate: 25000 },
  { id: '1080p', label: '1080p · 8 Mbps', width: 1920, height: 1080, bitrate: 8000 },
  { id: '720p', label: '720p · 4 Mbps', width: 1280, height: 720, bitrate: 4000 },
  { id: '480p', label: '480p · 1.5 Mbps', width: 854, height: 480, bitrate: 1500 },
] as const;

/** Applies quality and explicit decoder recovery without changing probed browser capabilities. */
export function playbackProfile(profile: ClientProfile, quality: PlaybackQuality, compatible: boolean): ClientProfile {
  const target = PLAYBACK_QUALITIES.find((entry) => entry.id === quality) ?? PLAYBACK_QUALITIES[0];
  return { ...profile, max_video_width: target.width, max_video_height: target.height,
    max_bitrate_kbps: target.bitrate, force_transcode: compatible };
}

/** Renders a keyboard-accessible quality selector for the current playback attempt. */
export function renderPlaybackQuality(quality: PlaybackQuality): string {
  const options = PLAYBACK_QUALITIES.map((entry) =>
    `<option value="${entry.id}" ${entry.id === quality ? 'selected' : ''}>${entry.label}</option>`).join('');
  return `<label class="player-quality">Quality <select id="player-quality" aria-label="Playback quality">${options}</select></label>`;
}

/** Labels direct delivery or server processing, including an alternate audio track. */
export function playbackDeliveryLabel(decision: PlaybackDecision, audioOverride = false): string {
  return decision.transcode_required || audioOverride ? 'Transcoding' : 'Direct Play';
}

/** Describes the active processing mode, showing both conversions when both tracks are encoded. */
export function playbackDeliveryDetail(decision: PlaybackDecision, audioOverride = false): string {
  const conversions: string[] = [];
  if (decision.video_transcode_required || audioOverride) conversions.push('Video conversion');
  if (decision.audio_transcode_required || audioOverride) conversions.push('Audio conversion');
  return conversions.join(' · ') || (decision.transcode_required ? 'Remuxing' : 'Direct Play');
}

/** Converts session or transport failures into useful actions without exposing server details. */
export function playbackRequestProblem(error: unknown): PlaybackProblem {
  if (error instanceof ApiError) {
    switch (error.status) {
      case 401: return { message: 'Your session has expired. Sign in again to continue playback.', retry: false, signIn: true };
      case 403: case 404: return { message: 'This media is unavailable or you no longer have access. Choose another item, or ask an administrator to check the library and source file.', retry: false };
      case 422: return { message: 'This item has no playable media. Ask an administrator to check the source file and rescan the library.', retry: false };
      case 503: return { message: 'The server cannot start playback. Retry, or ask an administrator to check FFmpeg in Transcoding settings.', retry: true };
      default: return { message: 'The server could not prepare playback. Retry, or ask an administrator to check the server logs.', retry: true };
    }
  }
  return { message: 'Cannot reach the server. Check your connection and retry playback.', retry: true };
}

/** Uses server conversion diagnostics when available, with native media-error fallbacks. */
export function mediaPlaybackProblem(code: number, compatible: boolean, status?: PlaybackSessionStatus): PlaybackProblem {
  if (status?.error) return { message: status.error.message, retry: true };
  if (code === 2) return { message: 'The stream was interrupted. Check your connection, choose a lower quality, or retry playback.', retry: true };
  if (code === 3 || code === 4) return {
    message: compatible
      ? 'The browser could not play the converted stream. Retry, or ask an administrator to check FFmpeg and the source file.'
      : 'The browser could not decode this stream. Try compatible playback to convert it on the server.',
    retry: true, compatible: !compatible,
  };
  return { message: 'Playback stopped before the media finished. Retry to continue from this position.', retry: true };
}

/** Renders escaped recovery text and only the actions appropriate to the failure. */
export function renderPlaybackProblem(problem?: PlaybackProblem): string {
  if (!problem) return '';
  return `<strong>Playback could not continue</strong><span>${escapeHtml(problem.message)}</span><div class="player-recovery-actions">
    ${problem.retry ? '<button type="button" data-player-retry>Retry playback</button>' : ''}
    ${problem.compatible ? '<button type="button" data-player-compatible>Try compatible playback</button>' : ''}
    ${problem.signIn ? '<button type="button" data-player-sign-in>Sign in again</button>' : ''}
    </div>`;
}
