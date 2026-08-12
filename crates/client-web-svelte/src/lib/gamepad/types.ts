// Gamepad types — shared across the gamepad package.

/** The 4 cardinal directions for D-pad / stick navigation. */
export type Direction = 'up' | 'down' | 'left' | 'right';

/**
 * Physical button/stick layout for a gamepad. Each field maps a logical
 * action to a raw button index or axis index. Non-standard controllers
 * (8BitDo Pro 3, etc.) have their own layouts; standard-mapping controllers
 * use the W3C layout.
 */
export interface GamepadLayout {
  /** Primary "confirm" button (A on Nintendo/Xbox, Cross on PlayStation). */
  confirm: number;
  /** "Cancel/back" button (B on Nintendo/Xbox, Circle on PlayStation). */
  cancel: number;
  /** L bumper (previous tab). */
  lBumper: number;
  /** R bumper (next tab). */
  rBumper: number;
  /** Select/Back/View/Share button (show help). */
  select: number;
  /** Start/Menu/Options button. */
  start: number;
  /** D-pad as buttons. If present, hatAxis is ignored. */
  dpadButtons?: { up: number; down: number; left: number; right: number };
  /** Hat axis index for D-pad (non-standard controllers). */
  hatAxis?: number;
  /** Neutral value for the hat axis (0 for most, 3.286 for 8BitDo Pro 3). */
  hatNeutral?: number;
  /** Left stick axes [x, y]. */
  leftStick: [number, number];
  /** Right stick axes [x, y]. */
  rightStick: [number, number];
}

/**
 * Normalized gamepad input — what the poller emits each frame.
 * All values are normalized regardless of the controller's raw layout.
 */
export interface GamepadInput {
  /** D-pad / hat directions (all false = neutral). */
  dpadUp: boolean;
  dpadDown: boolean;
  dpadLeft: boolean;
  dpadRight: boolean;
  /** Face buttons. */
  confirm: boolean;
  cancel: boolean;
  /** Shoulder bumpers. */
  lBumper: boolean;
  rBumper: boolean;
  /** Select / Start. */
  select: boolean;
  start: boolean;
  /** Left stick, normalized to [-1, 1] with deadzone applied. */
  leftStickX: number;
  leftStickY: number;
  /** Right stick, normalized to [-1, 1] with deadzone applied. */
  rightStickX: number;
  rightStickY: number;
  /** Raw button values (index → pressed), for extensibility. */
  rawButtons: boolean[];
}
