// Analog stick filtering — deadzone, normalization, edge detection.
// Pure functions — fully testable.

/**
 * Apply a radial deadzone to a stick value pair.
 * Values inside the deadzone radius are zeroed. Values outside are
 * renormalized so the output range is still [-1, 1] from the edge of the
 * deadzone to the full deflection.
 *
 * @param x Raw axis X (-1 to 1)
 * @param y Raw axis Y (-1 to 1)
 * @param deadzone Radius below which input is ignored (0 to 0.5, default 0.12)
 * @returns Filtered [x, y] with deadzone applied.
 */
export function applyDeadzone(x: number, y: number, deadzone = 0.12): [number, number] {
  const magnitude = Math.sqrt(x * x + y * y);
  if (magnitude < deadzone) {
    return [0, 0];
  }
  // Renormalize: map [deadzone, 1] → [0, 1] so the stick feels responsive
  // right at the edge of the deadzone.
  const normalized = Math.min(1, (magnitude - deadzone) / (1 - deadzone));
  const scale = normalized / magnitude;
  return [x * scale, y * scale];
}

/**
 * Check if a stick magnitude exceeds an activation threshold.
 * Used with hysteresis: the stick must exceed `activate` to trigger,
 * then drop below `release` before it can trigger again.
 *
 * @param magnitude Current stick magnitude (after deadzone)
 * @param prevState Whether the stick was "active" last frame
 * @param activate Threshold to enter active state (default 0.7)
 * @param release Threshold to leave active state (default 0.35)
 */
export function isStickActive(
  magnitude: number,
  prevState: boolean,
  activate = 0.7,
  release = 0.35,
): boolean {
  if (prevState) {
    return magnitude >= release;
  }
  return magnitude > activate;
}

/**
 * Determine the dominant direction of a stick input.
 * Returns the axis with the larger absolute value.
 */
export function stickDirection(x: number, y: number): 'horizontal' | 'vertical' | null {
  if (Math.abs(x) < 0.01 && Math.abs(y) < 0.01) return null;
  return Math.abs(x) > Math.abs(y) ? 'horizontal' : 'vertical';
}
