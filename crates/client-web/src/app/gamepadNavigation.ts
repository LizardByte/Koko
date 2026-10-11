import type { NavigationDirection } from './spatialNavigation';

export type GamepadNavigationAction = NavigationDirection | 'previous-section' | 'next-section' | 'activate' | 'back';

/** Decode standard D-pad/stick input without taking two steps for a diagonal. */
export function gamepadNavigationActions(gamepad: Pick<Gamepad, 'axes' | 'buttons'>): GamepadNavigationAction[] {
  const pressed = (index: number) => Boolean(gamepad.buttons[index]?.pressed);
  const actions: GamepadNavigationAction[] = [];
  const dpad = ([['up', 12], ['down', 13], ['left', 14], ['right', 15]] as const).find(([, index]) => pressed(index));
  if (dpad) {
    actions.push(dpad[0]);
  } else {
    const x = gamepad.axes[0] ?? 0;
    const y = gamepad.axes[1] ?? 0;
    if (Math.max(Math.abs(x), Math.abs(y)) >= 0.65) {
      actions.push(Math.abs(x) > Math.abs(y) ? (x > 0 ? 'right' : 'left') : (y > 0 ? 'down' : 'up'));
    }
  }
  if (pressed(4)) actions.push('previous-section');
  if (pressed(5)) actions.push('next-section');
  if (pressed(0)) actions.push('activate');
  if (pressed(1)) actions.push('back');
  return actions;
}

/** Repeat held directions with a delay; activation and section jumps fire once per press. */
export function createGamepadNavigationReader(): (
  gamepads: readonly (Gamepad | null)[], now: number, suspended: boolean,
) => GamepadNavigationAction[] {
  const held = new Map<string, number>();
  return (gamepads, now, suspended) => {
    const present = new Set<string>();
    const output: GamepadNavigationAction[] = [];
    for (const gamepad of gamepads) {
      if (!gamepad?.connected) continue;
      for (const action of gamepadNavigationActions(gamepad)) {
        const key = `${gamepad.index}:${action}`;
        present.add(key);
        const nextRepeat = held.get(key);
        const repeats = ['left', 'right', 'up', 'down'].includes(action);
        if (nextRepeat === undefined) {
          held.set(key, now + 400);
          if (!suspended) output.push(action);
        } else if (repeats && now >= nextRepeat) {
          held.set(key, now + 110);
          if (!suspended) output.push(action);
        }
      }
    }
    for (const key of held.keys()) {
      if (!present.has(key)) held.delete(key);
    }
    return output;
  };
}
