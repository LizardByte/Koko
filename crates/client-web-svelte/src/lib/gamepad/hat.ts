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
 * Each hat position mapped to its component cardinals. The 8 standard positions
 * are spaced at 1/7 ≈ 0.143 intervals; the upper bound of each range is exclusive
 * except the last. Index 0 is the deadzone (handled separately).
 *
 * Range order (low → high value): Up, UpRight, Right, DownRight, Down, DownLeft, Left, UpLeft.
 */
const HAT_RANGES: Array<{ max: number; dir: HatDirection }> = [
  { max: -0.85, dir: { up: true, down: false, left: false, right: false } },          // Up
  { max: -0.6, dir: { up: true, down: false, left: false, right: true } },           // UpRight
  { max: -0.3, dir: { up: false, down: false, left: false, right: true } },          // Right
  { max: -0.05, dir: { up: false, down: true, left: false, right: true } },          // DownRight
  { max: 0.05, dir: NEUTRAL },                                                       // deadzone
  { max: 0.3, dir: { up: false, down: true, left: false, right: false } },           // Down
  { max: 0.6, dir: { up: false, down: true, left: true, right: false } },            // DownLeft
  { max: 0.85, dir: { up: false, down: false, left: true, right: false } },          // Left
  { max: 1.05, dir: { up: true, down: false, left: true, right: false } },           // UpLeft
];

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
  // For neutral=0 (most controllers), the hat already has a natural gap around 0
  // — we use a tiny threshold (0.05) to filter noise. For non-zero neutrals
  // (8BitDo = 3.286), the full threshold is needed to distinguish from the
  // actual hat positions at [-1, 1].
  const effectiveThreshold = neutral === 0 ? 0.05 : threshold;
  if (Math.abs(value - neutral) < effectiveThreshold) return NEUTRAL;
  // Only map values in the [-1, 1] range (the 8 hat positions).
  if (value < -1.05 || value > 1.05) return NEUTRAL;
  // Find the first range whose max bound exceeds the value.
  return HAT_RANGES.find((range) => value <= range.max)?.dir ?? NEUTRAL;
}

/** True if any direction is active. */
export function hasHatDirection(hat: HatDirection): boolean {
  return hat.up || hat.down || hat.left || hat.right;
}

/**
 * Convert a hat direction to a single Direction (preferring the dominant axis).
 * Cardinal directions map directly; diagonals prefer horizontal (more common
 * for list navigation).
 */
export function hatToDirection(hat: HatDirection): Direction | undefined {
  if (!hasHatDirection(hat)) return undefined;
  // Diagonal — prefer horizontal (more common for list navigation).
  if (hat.left) return 'left';
  if (hat.right) return 'right';
  if (hat.up) return 'up';
  return 'down';
}
