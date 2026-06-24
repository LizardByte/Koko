import { describe, it, expect } from 'vitest';
import { parseHat, hasHatDirection, hatToDirection } from '../hat';

describe('parseHat', () => {
  it('returns neutral for value 0 with neutral=0', () => {
    const result = parseHat(0, 0);
    expect(result.up).toBe(false);
    expect(result.down).toBe(false);
    expect(result.left).toBe(false);
    expect(result.right).toBe(false);
  });

  it('returns neutral for value near neutral (within threshold)', () => {
    const result = parseHat(0.03, 0, 0.2);
    expect(hasHatDirection(result)).toBe(false);
  });

  it('returns Up for value -1', () => {
    const result = parseHat(-1, 0);
    expect(result.up).toBe(true);
    expect(result.right).toBe(false);
  });

  it('returns Up+Right for value -0.714 (UpRight diagonal)', () => {
    const result = parseHat(-0.714, 0);
    expect(result.up).toBe(true);
    expect(result.right).toBe(true);
  });

  it('returns Right for value -0.429', () => {
    const result = parseHat(-0.429, 0);
    expect(result.right).toBe(true);
    expect(result.up).toBe(false);
  });

  it('returns Down+Right for value -0.143 (DownRight diagonal)', () => {
    const result = parseHat(-0.143, 0);
    expect(result.down).toBe(true);
    expect(result.right).toBe(true);
  });

  it('returns Down for value 0.143', () => {
    const result = parseHat(0.143, 0);
    expect(result.down).toBe(true);
  });

  it('returns Down+Left for value 0.429 (DownLeft diagonal)', () => {
    const result = parseHat(0.429, 0);
    expect(result.down).toBe(true);
    expect(result.left).toBe(true);
  });

  it('returns Left for value 0.714', () => {
    const result = parseHat(0.714, 0);
    expect(result.left).toBe(true);
  });

  it('returns Up+Left for value 1.0 (UpLeft diagonal)', () => {
    const result = parseHat(1.0, 0);
    expect(result.up).toBe(true);
    expect(result.left).toBe(true);
  });

  it('returns neutral for 8BitDo Pro 3 neutral value 3.286', () => {
    const result = parseHat(3.286, 3.286, 0.2);
    expect(hasHatDirection(result)).toBe(false);
  });

  it('returns neutral for values outside [-1.05, 1.05] range', () => {
    expect(hasHatDirection(parseHat(2.0, 0))).toBe(false);
    expect(hasHatDirection(parseHat(-2.0, 0))).toBe(false);
    expect(hasHatDirection(parseHat(3.286, 0))).toBe(false);
  });

  it('returns Up for value -1 with 8BitDo neutral', () => {
    const result = parseHat(-1, 3.286, 0.2);
    expect(result.up).toBe(true);
  });
});

describe('hatToDirection', () => {
  it('returns the dominant cardinal for pure directions', () => {
    expect(hatToDirection(parseHat(-1, 0))).toBe('up');
    expect(hatToDirection(parseHat(0.143, 0))).toBe('down');
    expect(hatToDirection(parseHat(0.714, 0))).toBe('left');
    expect(hatToDirection(parseHat(-0.429, 0))).toBe('right');
  });

  it('prefers horizontal for diagonals', () => {
    expect(hatToDirection(parseHat(-0.714, 0))).toBe('right');
    expect(hatToDirection(parseHat(0.429, 0))).toBe('left');
  });

  it('returns undefined for neutral', () => {
    expect(hatToDirection(parseHat(0, 0))).toBeUndefined();
  });
});
