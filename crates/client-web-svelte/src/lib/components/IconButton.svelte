<script lang="ts">
  // IconButton — a square, icon-only button with no label. Tailored for the
  // icon-only case (fixed 2.35rem size, centered icon) rather than being a
  // Boolean prop on <Button>. Shares variant/anchor logic with <Button> via
  // ./button-types.ts so the two stay in sync.
  //
  // Use <Button> when you have a label (with or without an icon). Use
  // <IconButton> when the affordance is purely iconographic (close, scan,
  // move-up/down, transport controls). The `title`/`aria-label` prop carries
  // the accessible name since there's no visible text.
  import Icon from './Icon.svelte';
  import './button.css';
  import { buttonClasses, type ButtonVariant } from './button-types';

  type Props = {
    /** Icon name (required — this is an icon button). */
    icon: string;
    /** Accessible name — required when no title is set. */
    label?: string;
    /** Defaults to 16 (icon-only buttons use a slightly smaller icon). */
    iconSize?: number;
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
  };

  let {
    icon,
    label,
    iconSize = 16,
    variant = 'secondary',
    busy = false,
    disabled = false,
    type = 'button',
    id,
    class: className = '',
    title,
    href,
    target,
    rel,
    onclick,
    ...rest
  }: Props = $props();

  const classes = $derived(buttonClasses(variant, className, 'icon-only-button'));
</script>

{#if href}
  <a
    {href}
    {id}
    {title}
    {target}
    {rel}
    class="button-link {classes}"
    aria-label={label}
    aria-busy={busy}
    aria-disabled={disabled || undefined}
    role={disabled ? 'link' : undefined}
    tabindex={disabled ? -1 : undefined}
    {onclick}
    {...rest}
  >
    <Icon name={icon} size={iconSize} />
  </a>
{:else}
  <button
    {type}
    {id}
    {title}
    {disabled}
    class={classes}
    class:is-busy={busy}
    aria-busy={busy}
    aria-label={label}
    {onclick}
    {...rest}
  >
    <Icon name={icon} size={iconSize} />
  </button>
{/if}
