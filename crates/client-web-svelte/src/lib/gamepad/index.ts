// Gamepad package — barrel exports.
// This package is designed to be extractable as a standalone library.
export type { GamepadLayout, GamepadInput, Direction } from './types';
export { detectLayout, STANDARD_LAYOUT, BITDO_PRO_3_LAYOUT, SWITCH_PRO_LAYOUT, DUALSENSE_LAYOUT, STEAM_DECK_LAYOUT } from './layouts';
export { parseHat, hasHatDirection, hatToDirection } from './hat';
export type { HatDirection } from './hat';
export { applyDeadzone, isStickActive, stickDirection } from './stick';
export { readGamepad, startPolling } from './poller';
export type { InputCallback } from './poller';
