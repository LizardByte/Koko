import { describe, it, expect } from 'vitest';
import { detectLayout, BITDO_PRO_3_LAYOUT } from '../layouts';

function mockGamepad(id: string, mapping: string): Gamepad {
  return {
    id,
    index: 0,
    mapping,
    connected: true,
    timestamp: 0,
    axes: [],
    buttons: [],
  } as unknown as Gamepad;
}

describe('detectLayout', () => {
  it('returns standard layout for mapping="standard"', () => {
    const layout = detectLayout(mockGamepad('Xbox Controller', 'standard'));
    expect(layout.confirm).toBe(0);
    expect(layout.cancel).toBe(1);
  });

  it('returns 8BitDo Pro 3 layout for the matching device', () => {
    const layout = detectLayout(mockGamepad('8BitDo Pro 3 (Vendor: 2dc8 Product: 6009)', ''));
    expect(layout).toEqual(BITDO_PRO_3_LAYOUT);
    expect(layout.confirm).toBe(1);
    expect(layout.cancel).toBe(0);
    expect(layout.hatAxis).toBe(9);
    expect(layout.hatNeutral).toBe(3.286);
    expect(layout.rightStick).toEqual([2, 5]);
  });

  it('falls back to standard for unknown non-standard controller', () => {
    const layout = detectLayout(mockGamepad('Unknown Controller', ''));
    expect(layout.confirm).toBe(0);
    expect(layout.cancel).toBe(1);
  });

  it('returns standard for DualSense in standard mode', () => {
    const layout = detectLayout(mockGamepad('DualSense Wireless Controller', 'standard'));
    expect(layout.confirm).toBe(0);
  });
});
