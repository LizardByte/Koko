<script lang="ts">
  // Button — the design-system button. Renders a <button> by default, or an
  // <a> when `href` is provided (so external links can share the same visual
  // variants without a separate "button-link" affordance). Replaces
  // renderButtonContent() (../client-web/src/app/ui.ts:60-71) + the
  // vanilla `.button-link` class.
  //
  // Variant classes (.secondary-button / .danger-button / .button-link) live
  // in ./button.css — co-located with their only emitter. SonarCloud's CSS
  // analyzer does not scan .svelte files, so moving them out of app.css
  // eliminates the false-positive contrast warnings. The bare `button {}`
  // element reset stays global.
  import Icon from './Icon.svelte';
  import './button.css';
  import type { Snippet } from 'svelte';

  type Props = {
    icon?: string;
    iconPosition?: 'start' | 'end';
    label?: string;
    variant?: 'primary' | 'secondary' | 'danger';
    busy?: boolean;
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    id?: string;
    class?: string;
    title?: string;
    /** When set, renders an <a> with this href instead of a <button>. */
    href?: string;
    /** Anchor target (only used with href). */
    target?: string;
    /** Anchor rel (only used with href). */
    rel?: string;
    onclick?: (event: MouseEvent) => void;
    children?: Snippet;
    [key: `data-${string}`]: string | undefined;
  };

  let {
    icon,
    iconPosition = 'start',
    label,
    variant = 'primary',
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
    children,
    ...rest
  }: Props = $props();

  const VARIANT_CLASS: Record<NonNullable<Props['variant']>, string> = {
    primary: '',
    secondary: 'secondary-button',
    danger: 'danger-button',
  };
  const variantClass = $derived(VARIANT_CLASS[variant]);
</script>

{#if href}
  <a
    {href}
    {id}
    {title}
    {target}
    {rel}
    class="button-link {variantClass} {className}"
    aria-busy={busy}
    aria-disabled={disabled || undefined}
    role={disabled ? 'link' : undefined}
    tabindex={disabled ? -1 : undefined}
    {onclick}
    {...rest}
  >
    {#if icon}
      <span class="button-icon"><Icon name={icon} /></span>
    {/if}
    {#if label}<span class="button-label">{label}</span>{/if}
    {#if children}{@render children()}{/if}
  </a>
{:else}
  <button
    {type}
    {id}
    {title}
    {disabled}
    class="{variantClass} {className}"
    class:is-busy={busy}
    aria-busy={busy}
    {onclick}
    {...rest}
  >
    {#if icon}
      <span class="button-icon"><Icon name={icon} /></span>
    {/if}
    {#if label}<span class="button-label">{label}</span>{/if}
    {#if children}{@render children()}{/if}
  </button>
{/if}
