/** Detects stalled TV playback and limits retries across reconnects to the same buffer. */

/** A playback observation, independent of browser readiness events. */
export interface TvPlaybackObservation {
  /** Absolute position within the recording buffer, in milliseconds. */
  position: number;
  paused: boolean;
  interrupted: boolean;
  unavailable: boolean;
}

/** Keeps retry history when a decoder is replaced, and forgives failures after sustained playback. */
export class TvPlaybackWatchdog {
  attempts = 0;
  private lastAttempt = Number.NEGATIVE_INFINITY;
  private lastPosition = 0;
  private lastProgress = 0;
  private healthySince: number | undefined;

  /** Give a newly connected decoder time to start; position and clock values use milliseconds. */
  attach(position: number, now: number): void {
    this.lastPosition = position;
    this.lastProgress = now;
    this.healthySince = undefined;
  }

  /** Reconnect after twelve seconds without progress, including freezes with a healthy readyState. */
  observe(observation: TvPlaybackObservation, now: number): 'recover' | 'exhausted' | undefined {
    const advanced = observation.position > this.lastPosition + 250;
    this.lastPosition = observation.position;
    if (observation.paused || observation.unavailable) {
      this.lastProgress = now;
      this.healthySince = undefined;
      return;
    }
    if (advanced && !observation.interrupted) {
      this.lastProgress = now;
      this.healthySince ??= now;
      if (now - this.healthySince >= 30000) this.attempts = 0;
      return;
    }
    this.healthySince = undefined;
    if ((!observation.interrupted && now - this.lastProgress < 12000) || now - this.lastAttempt < 15000) return;
    if (this.attempts >= 3) return 'exhausted';
    this.attempts++;
    this.lastAttempt = now;
    return 'recover';
  }
}
