import { describe, it, expect } from 'vitest';
import { applyDeadzone, isStickActive, stickDirection } from '../stick';

describe('applyDeadzone', () => {
  it('zeroes values inside the deadzone', () => {
    const [x, y] = applyDeadzone(0.05, 0.05, 0.12);
    expect(x).toBe(0);
    expect(y).toBe(0);
  });

  it('zeroes values at exactly the deadzone boundary', () => {
    const [x, y] = applyDeadzone(0.12, 0, 0.12);
    expect(x).toBe(0);
    expect(y).toBe(0);
  });

  it('renormalizes values outside the deadzone', () => {
    const [x, y] = applyDeadzone(1, 0, 0.12);
    expect(x).toBeCloseTo(1);
    expect(y).toBeCloseTo(0);
  });

  it('renormalizes a mid-range value correctly', () => {
    // magnitude = 0.5, deadzone = 0.12
    // normalized = (0.5 - 0.12) / (1 - 0.12) = 0.432
    const [x, y] = applyDeadzone(0.5, 0, 0.12);
    expect(x).toBeCloseTo(0.4318, 3);
    expect(y).toBeCloseTo(0);
  });

  it('handles diagonal inputs', () => {
    const [x, y] = applyDeadzone(0.5, 0.5, 0.12);
    // magnitude = sqrt(0.25 + 0.25) = 0.707
    // normalized = (0.707 - 0.12) / (1 - 0.12) = 0.667
    // scale = 0.667 / 0.707 = 0.943
    expect(x).toBeCloseTo(0.5 * 0.943, 2);
    expect(y).toBeCloseTo(0.5 * 0.943, 2);
  });

  it('zeroes zero input', () => {
    const [x, y] = applyDeadzone(0, 0, 0.12);
    expect(x).toBe(0);
    expect(y).toBe(0);
  });
});

describe('isStickActive', () => {
  it('activates when magnitude exceeds activate threshold', () => {
    expect(isStickActive(0.8, false, 0.7, 0.35)).toBe(true);
  });

  it('does not activate when magnitude is below activate threshold', () => {
    expect(isStickActive(0.5, false, 0.7, 0.35)).toBe(false);
  });

  it('stays active when magnitude is between release and activate (hysteresis)', () => {
    expect(isStickActive(0.5, true, 0.7, 0.35)).toBe(true);
  });

  it('deactivates when magnitude drops below release threshold', () => {
    expect(isStickActive(0.2, true, 0.7, 0.35)).toBe(false);
  });

  it('stays active at exactly the release threshold', () => {
    expect(isStickActive(0.35, true, 0.7, 0.35)).toBe(true);
  });
});

describe('stickDirection', () => {
  it('returns horizontal for X-dominant input', () => {
    expect(stickDirection(0.8, 0.2)).toBe('horizontal');
  });

  it('returns vertical for Y-dominant input', () => {
    expect(stickDirection(0.2, 0.8)).toBe('vertical');
  });

  it('returns null for zero input', () => {
    expect(stickDirection(0, 0)).toBeNull();
  });

  it('returns vertical when X and Y are equal (Y wins ties)', () => {
    expect(stickDirection(0.5, 0.5)).toBe('vertical');
  });
});
