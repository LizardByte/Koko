import type { NavigationDirection } from './spatialNavigation';

export type GamepadNavigationAction = NavigationDirection | 'previous-section' | 'next-section' | 'activate' | 'back';

const DPAD_BUTTONS = [['up', 12], ['down', 13], ['left', 14], ['right', 15]] as const;
const ACTION_BUTTONS = [['previous-section', 4], ['next-section', 5], ['activate', 0], ['back', 1]] as const;
const REPEATING_ACTIONS = new Set<GamepadNavigationAction>(['left', 'right', 'up', 'down']);

/** Resolve only the dominant stick axis after it passes the navigation dead zone. */
function leftStickDirection(axes: readonly number[]): NavigationDirection | undefined {
  const x = axes[0] ?? 0;
  const y = axes[1] ?? 0;
  if (Math.max(Math.abs(x), Math.abs(y)) >= 0.65) {
    if (Math.abs(x) > Math.abs(y)) return x > 0 ? 'right' : 'left';
    return y > 0 ? 'down' : 'up';
  }
  return undefined;
}

/** Decode standard D-pad/stick input without taking two steps for a diagonal. */
export function gamepadNavigationActions(gamepad: Pick<Gamepad, 'axes' | 'buttons'>): GamepadNavigationAction[] {
  const pressed = (index: number) => Boolean(gamepad.buttons[index]?.pressed);
  const actions: GamepadNavigationAction[] = [];
  const dpad = DPAD_BUTTONS.find(([, index]) => pressed(index));
  const direction = dpad?.[0] ?? leftStickDirection(gamepad.axes);
  if (direction) actions.push(direction);
  for (const [action, index] of ACTION_BUTTONS) {
    if (pressed(index)) actions.push(action);
  }
  return actions;
}

/** Record a held action and decide whether this poll should emit it. */
function actionIsDue(held: Map<string, number>, key: string, action: GamepadNavigationAction, now: number): boolean {
  const nextRepeat = held.get(key);
  if (nextRepeat === undefined) {
    held.set(key, now + 400);
    return true;
  }
  if (REPEATING_ACTIONS.has(action) && now >= nextRepeat) {
    held.set(key, now + 110);
    return true;
  }
  return false;
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
        if (actionIsDue(held, key, action, now) && !suspended) output.push(action);
      }
    }
    for (const key of held.keys()) {
      if (!present.has(key)) held.delete(key);
    }
    return output;
  };
}
