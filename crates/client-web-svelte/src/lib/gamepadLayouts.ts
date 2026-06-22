// Gamepad layout detection — maps physical controls to logical actions
// based on the connected device. The W3C "standard" mapping defines a known
// layout, but many controllers (8BitDo Pro 3, etc.) report mapping: "" with
// raw indices. This file provides known layouts for common devices + falls
// back to the standard mapping when no match is found.

export type GamepadLayout = {
  /** Button index for the primary "confirm" button (A on Nintendo/Xbox). */
  confirm: number;
  /** Button index for the "back/cancel" button (B on Nintendo, B on Xbox). */
  cancel: number;
  /** L bumper (previous tab). */
  lBumper: number;
  /** R bumper (next tab). */
  rBumper: number;
  /** Select/Back button (show help). */
  select: number;
  /** D-pad: if null, D-pad is on the hatAxis. */
  dpadButtons?: { up: number; down: number; left: number; right: number };
  /** Hat axis index for D-pad (non-standard controllers). Null if D-pad is buttons. */
  hatAxis?: number;
  /** Neutral value for the hat axis (0 for most controllers, 3.286 for 8BitDo Pro 3). */
  hatNeutral?: number;
  /** Left stick axes [x, y]. */
  leftStick: [number, number];
  /** Right stick axes [x, y]. */
  rightStick: [number, number];
};

/** Standard W3C mapping layout (Xbox/PS5/DualSense in standard mode). */
const STANDARD_LAYOUT: GamepadLayout = {
  confirm: 0,
  cancel: 1,
  lBumper: 4,
  rBumper: 5,
  select: 8,
  dpadButtons: { up: 12, down: 13, left: 14, right: 15 },
  leftStick: [0, 1],
  rightStick: [2, 3],
};

/**
 * 8BitDo Pro 3 (Vendor: 2dc8 Product: 6009).
 * Nintendo-style face layout (A=right, B=bottom — swapped from Xbox).
 * D-pad is a hat on axis 9, not buttons. 24 buttons, 10 axes.
 *
 * Hat axis 9 values (probed):
 *   Neutral = 3.286 (NOT 0!)
 *   Up = -1, UpRight = -0.714, Right = -0.429, DownRight = -0.143,
 *   Down = 0.143, DownLeft = 0.429, Left = 0.714, UpLeft = 1
 *
 * Probed button order:
 *   0=B(cancel), 1=A(confirm), 2=PR, 3=Y, 4=X,
 *   5=PL, 6=L bumper, 7=R bumper, 8=L2 trigger, 9=R2 trigger,
 *   10=Select, 11=Start, 12=Home(opens macOS Games),
 *   13=L-stick click, 14=R-stick click,
 *   16=L4, 17=R4
 *
 * Axis layout:
 *   0,1 = left stick (X, Y)
 *   2 = right stick X
 *   3,4 = L2/R2 analog triggers (only negative, -1 to 0)
 *   5 = right stick Y
 *   9 = D-pad hat (neutral = 3.286)
 */
const BITDO_PRO_3_LAYOUT: GamepadLayout = {
  confirm: 1,   // A (right face button in Nintendo layout)
  cancel: 0,    // B (bottom face button)
  lBumper: 6,   // L bumper
  rBumper: 7,   // R bumper
  select: 10,   // Select/Back
  hatAxis: 9,   // D-pad on hat axis 9
  hatNeutral: 3.286, // Neutral value is NOT 0!
  leftStick: [0, 1],
  rightStick: [2, 5], // Right stick Y is axis 5, NOT 3 (axis 3 = L2 trigger)
};

/** Known device layouts, matched by the gamepad ID string. */
const KNOWN_DEVICES: Array<{ match: RegExp; layout: GamepadLayout }> = [
  { match: /8BitDo Pro 3/i, layout: BITDO_PRO_3_LAYOUT },
  // Add more devices as probed.
];

/**
 * Detect the best layout for a connected gamepad. Falls back to the standard
 * W3C mapping when the gamepad uses mapping: "standard" or no known device
 * matches.
 */
export function detectLayout(gamepad: Gamepad): GamepadLayout {
  // If the browser reports standard mapping, use the standard layout.
  if (gamepad.mapping === 'standard') {
    return STANDARD_LAYOUT;
  }

  // Check known devices by ID.
  for (const device of KNOWN_DEVICES) {
    if (device.match.test(gamepad.id)) {
      return device.layout;
    }
  }

  // Fallback: standard layout (best guess for unknown controllers).
  return STANDARD_LAYOUT;
}
