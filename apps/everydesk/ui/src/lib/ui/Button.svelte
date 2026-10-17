<!--
  Button.
  Props:
    variant?: 'primary' | 'neutral' | 'ghost' | 'danger' = 'ghost'
      primary is the accent ("Conectar") — use at most one per screen.
      neutral is a filled default without the accent: the safe choice when the accent would
      push toward a risk (e.g. "Recusar" on a card whose key does not match).
    size?: 'md' | 'lg' = 'md'
    loading?: boolean — keeps the label, draws one line of light along the bottom edge that
      advances once and stays (no loop, no spinner), sets aria-busy and blocks clicks.
    disabled?: boolean
    type?: 'button' | 'submit' = 'button'
    icon?: lucide component shown before the label
    label?: string — accessible name when the button has only an icon
    onclick?: (e: MouseEvent) => void
    children?: Snippet — the visible label
    ...rest — other attributes for the <button> (aria-expanded, aria-controls, data-*)
-->
<script lang="ts">
  import type { ComponentType, Snippet, SvelteComponent } from 'svelte'
  import type { HTMLButtonAttributes } from 'svelte/elements'
  import type { IconProps } from 'lucide-svelte'
  import Icon from './Icon.svelte'

  interface Props extends Omit<HTMLButtonAttributes, 'type' | 'onclick' | 'children'> {
    variant?: 'primary' | 'neutral' | 'ghost' | 'danger'
    size?: 'md' | 'lg'
    loading?: boolean
    disabled?: boolean
    type?: 'button' | 'submit'
    icon?: ComponentType<SvelteComponent<IconProps>>
    label?: string
    onclick?: (e: MouseEvent) => void
    children?: Snippet
  }
  let {
    variant = 'ghost',
    size = 'md',
    loading = false,
    disabled = false,
    type = 'button',
    icon,
    label,
    onclick,
    children,
    class: extra = '',
    ...rest
  }: Props = $props()

  function click(e: MouseEvent) {
    if (loading) {
      e.preventDefault()
      return
    }
    onclick?.(e)
  }
</script>

<button
  {...rest}
  class="btn {variant} {size} {extra}"
  class:icon-only={!children}
  {type}
  {disabled}
  aria-label={label}
  aria-busy={loading || undefined}
  aria-disabled={loading || undefined}
  onclick={click}
>
  {#if icon}<Icon {icon} size={size === 'lg' ? 22 : 20} />{/if}
  {#if children}<span class="text">{@render children()}</span>{/if}
  {#if loading}<span class="trace" aria-hidden="true"></span>{/if}
</button>

<style>
  .btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    overflow: hidden;
    border: 1px solid transparent;
    border-radius: var(--radius-pill);
    font: inherit;
    font-family: var(--font-ui);
    font-weight: 600;
    letter-spacing: var(--track-label);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }
  .md {
    min-height: 44px;
    padding: 0 var(--space-5);
    font-size: var(--fs-body);
  }
  /* 64px: level with the Field input it usually sits next to */
  .lg {
    min-height: 64px;
    padding: 0 var(--space-6);
    font-size: var(--fs-lead);
  }
  .icon-only.md {
    width: 44px;
    padding: 0;
  }
  .icon-only.lg {
    width: 64px;
    padding: 0;
  }

  .primary {
    background: var(--accent);
    color: var(--on-accent);
  }
  .primary:hover:not(:disabled) {
    box-shadow: 0 0 0 6px var(--accent-track), 0 0 32px var(--accent-glow);
  }

  .neutral {
    background: var(--text);
    color: var(--bg);
  }
  .neutral:hover:not(:disabled) {
    box-shadow: 0 0 0 6px var(--surface-2);
  }

  .ghost {
    background: transparent;
    border-color: var(--line-strong);
    color: var(--text);
  }
  .ghost.icon-only {
    border-color: transparent;
  }
  .ghost:hover:not(:disabled) {
    background: var(--surface-2);
  }

  .danger {
    background: var(--danger-soft);
    border-color: var(--danger);
    color: var(--danger);
  }
  .danger:hover:not(:disabled) {
    background: var(--danger);
    color: var(--bg);
  }

  .btn:active:not(:disabled) {
    transform: scale(0.97);
  }
  .btn:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
  }
  .btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
  .btn[aria-busy='true'] {
    cursor: progress;
  }

  /* loading: one line of light advances along the bottom edge and stays there. It runs
     once (no loop: a repeating cue would be a spinner, spec §6.2). */
  .trace {
    position: absolute;
    left: 0;
    bottom: 0;
    width: 100%;
    height: 2px;
    border-radius: 2px;
    background: currentColor;
    opacity: 0.6;
    transform-origin: left center;
    animation: trace 1.2s var(--ease-out) 1 both;
  }
  @keyframes trace {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .trace {
      animation: none;
    }
  }
</style>
