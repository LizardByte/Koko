// Shared button types — used by Button.svelte + IconButton.svelte so the two
// components stay in sync without one extending the other's Props.

export type ButtonVariant = 'primary' | 'secondary' | 'danger';

/** Props common to both Button and IconButton (element + variant + state). */
export interface ButtonBaseProps {
  variant?: ButtonVariant;
  busy?: boolean;
  disabled?: boolean;
  type?: 'button' | 'submit' | 'reset';
  id?: string;
  class?: string;
  title?: string;
  href?: string;
  target?: string;
  rel?: string;
  onclick?: (event: MouseEvent) => void;
  [key: `data-${string}`]: string | undefined;
}

/** Variant → CSS class. Shared so both components resolve identically. */
export const VARIANT_CLASS: Record<ButtonVariant, string> = {
  primary: 'primary-button',
  secondary: 'secondary-button',
  danger: 'danger-button',
};

/**
 * Build the class string for a button element: variant class + icon-only
 * modifier (when applicable) + caller's className.
 */
export function buttonClasses(variant: ButtonVariant, className = '', extra = ''): string {
  return [VARIANT_CLASS[variant], extra, className].filter(Boolean).join(' ');
}
