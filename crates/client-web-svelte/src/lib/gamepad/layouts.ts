// Gamepad layout detection — maps physical controls to logical actions
// based on the connected device.

import type { GamepadLayout } from './types';

/** Standard W3C mapping (Xbox, DualSense in standard mode, most PC controllers). */
export const STANDARD_LAYOUT: GamepadLayout = {
  confirm: 0,    // A / Cross
  cancel: 1,     // B / Circle
  lBumper: 4,    // LB / L1
  rBumper: 5,    // RB / R1
  select: 8,     // Back / View / Share / Create
  start: 9,      // Start / Menu / Options
  dpadButtons: { up: 12, down: 13, left: 14, right: 15 },
  leftStick: [0, 1],
  rightStick: [2, 3],
};

/**
 * 8BitDo Pro 3 (Vendor: 2dc8 Product: 6009).
 * Nintendo-style face layout (A=1, B=0 — swapped from Xbox standard).
 * D-pad is a hat on axis 9 with neutral = 3.286. 24 buttons, 10 axes.
 * Right stick Y is axis 5 (not 3, which is L2 trigger).
 */
export const BITDO_PRO_3_LAYOUT: GamepadLayout = {
  confirm: 1,
  cancel: 0,
  lBumper: 6,
  rBumper: 7,
  select: 10,
  start: 11,
  hatAxis: 9,
  hatNeutral: 3.286,
  leftStick: [0, 1],
  rightStick: [2, 5],
};

/** Nintendo Switch Pro Controller (standard mapping on most browsers). */
export const SWITCH_PRO_LAYOUT: GamepadLayout = {
  // Nintendo layout: A=1, B=0, X=3, Y=2 in raw mode, but most browsers
  // remap to standard when mapping="standard".
  ...STANDARD_LAYOUT,
};

/** PlayStation DualSense (PS5) — standard mapping. */
export const DUALSENSE_LAYOUT: GamepadLayout = {
  ...STANDARD_LAYOUT,
  // Cross=0, Circle=1 match standard confirm/cancel for most regions.
};

/** Steam Deck built-in controller — standard mapping. */
export const STEAM_DECK_LAYOUT: GamepadLayout = {
  ...STANDARD_LAYOUT,
};

/** Known device layouts, matched by the gamepad ID string. */
const KNOWN_DEVICES: Array<{ match: RegExp; layout: GamepadLayout }> = [
  { match: /8BitDo Pro 3/i, layout: BITDO_PRO_3_LAYOUT },
  // Add more as probed. Standard-mapping controllers don't need entries.
];

/**
 * Detect the best layout for a connected gamepad.
 * Falls back to the standard W3C mapping when the gamepad uses
 * mapping: "standard" or no known device matches.
 */
export function detectLayout(gamepad: Gamepad): GamepadLayout {
  if (gamepad.mapping === 'standard') {
    return STANDARD_LAYOUT;
  }

  for (const device of KNOWN_DEVICES) {
    if (device.match.test(gamepad.id)) {
      return device.layout;
    }
  }

  return STANDARD_LAYOUT;
}
