// Hat-axis → direction parsing.
// Pure functions — fully testable without a DOM or gamepad.

import type { Direction } from './types';

export type HatDirection = {
  up: boolean;
  down: boolean;
  left: boolean;
  right: boolean;
};

const NEUTRAL: HatDirection = { up: false, down: false, left: false, right: false };

/**
 * Parse a hat axis value into cardinal directions.
 *
 * Standard 8-position hat values (divisions of 1/7):
 *   Up=-1, UpRight=-0.714, Right=-0.429, DownRight=-0.143,
 *   Down=0.143, DownLeft=0.429, Left=0.714, UpLeft=1, Neutral=custom
 *
 * Each diagonal activates both component cardinals.
 * Values near the neutral value return all-false.
 *
 * @param value The raw axis value
 * @param neutral The neutral/resting value (0 for most controllers, 3.286 for 8BitDo)
 * @param threshold How far from neutral before activating (default 0.2)
 */
export function parseHat(value: number, neutral = 0, threshold = 0.2): HatDirection {
  // Check distance from neutral. For neutral=0 (most controllers), the hat
  // already has a natural gap around 0 — we use a tiny threshold (0.05) to
  // filter noise. For non-zero neutrals (8BitDo = 3.286), the full threshold
  // is needed to distinguish from the actual hat positions at [-1, 1].
  const effectiveThreshold = neutral === 0 ? 0.05 : threshold;
  if (Math.abs(value - neutral) < effectiveThreshold) {
    return NEUTRAL;
  }

  // Only map values in the [-1, 1] range (the 8 hat positions).
  if (value < -1.05 || value > 1.05) {
    return NEUTRAL;
  }

  const result: HatDirection = { up: false, down: false, left: false, right: false };

  // Each hat position maps to a tight range. The 8 standard positions are
  // spaced at 1/7 ≈ 0.143 intervals. We use ±0.07 around each position.
  // Note: the gap around 0 (between -0.07 and 0.07) is already covered by
  // the neutral check above for controllers with neutral=0.
  // Up: -1.0
  if (value <= -0.85) result.up = true;
  // UpRight: -0.714
  else if (value <= -0.6 && value > -0.85) { result.up = true; result.right = true; }
  // Right: -0.429
  else if (value <= -0.3 && value > -0.6) result.right = true;
  // DownRight: -0.143
  else if (value <= -0.05 && value > -0.3) { result.down = true; result.right = true; }
  // Down: 0.143
  else if (value >= 0.05 && value < 0.3) result.down = true;
  // DownLeft: 0.429
  else if (value >= 0.3 && value < 0.6) { result.down = true; result.left = true; }
  // Left: 0.714
  else if (value >= 0.6 && value < 0.85) result.left = true;
  // UpLeft: 1.0
  else if (value >= 0.85 && value <= 1.05) { result.up = true; result.left = true; }
  // Values in the tiny gap around 0 but outside threshold → no direction.

  return result;
}

/** True if any direction is active. */
export function hasHatDirection(hat: HatDirection): boolean {
  return hat.up || hat.down || hat.left || hat.right;
}

/** Convert a hat direction to a single Direction (preferring the dominant axis). */
export function hatToDirection(hat: HatDirection): Direction | undefined {
  if (hat.up && !hat.left && !hat.right) return 'up';
  if (hat.down && !hat.left && !hat.right) return 'down';
  if (hat.left && !hat.up && !hat.down) return 'left';
  if (hat.right && !hat.up && !hat.down) return 'right';
  // Diagonal — pick the horizontal (more common for list navigation)
  if (hat.left) return 'left';
  if (hat.right) return 'right';
  if (hat.up) return 'up';
  if (hat.down) return 'down';
  return undefined;
}
