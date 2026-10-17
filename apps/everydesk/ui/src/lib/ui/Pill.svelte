<!--
  Pill — floating container for the in-session controls (spec §6.2 moment 3).
  Collapsed it shows a short summary; on hover or keyboard focus it expands into tools and
  collapses by itself after `idleMs` without interaction. Pointer and wheel events never
  reach what lies underneath (the remote screen).
  Props:
    expanded?: boolean (bindable) = false
    idleMs?: number = 2000
    label: string — accessible name of the group, e.g. "Controles da sessão"
    summary: Snippet — always visible (latency, contact name, voice dot)
    children?: Snippet — the tools shown when expanded
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import { onDestroy } from 'svelte'
  import { createIdleTimer } from './pill'

  interface Props {
    expanded?: boolean
    idleMs?: number
    label: string
    summary: Snippet
    children?: Snippet
  }
  let { expanded = $bindable(false), idleMs = 2000, label, summary, children }: Props = $props()

  let root: HTMLDivElement
  let pointerInside = false
  let focusInside = false

  const timer = createIdleTimer(() => idleMs, () => {
    expanded = false
  })
  onDestroy(() => timer.cancel())

  function open() {
    expanded = true
    timer.hold()
  }
  function maybeRelease() {
    if (!pointerInside && !focusInside) timer.release()
  }
  function onpointerenter() {
    pointerInside = true
    open()
  }
  function onpointerleave() {
    pointerInside = false
    maybeRelease()
  }
  function onfocusin() {
    focusInside = true
    open()
  }
  function onfocusout(e: FocusEvent) {
    if (root.contains(e.relatedTarget as Node | null)) return
    focusInside = false
    maybeRelease()
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && expanded) {
      e.stopPropagation()
      expanded = false
      timer.cancel()
    }
  }
  /** Keep pointer input on the pill from reaching the remote screen below. */
  const absorb = (e: Event) => e.stopPropagation()
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={root}
  class="pill"
  class:expanded
  role="toolbar"
  tabindex="-1"
  aria-label={label}
  {onpointerenter}
  {onpointerleave}
  {onfocusin}
  {onfocusout}
  {onkeydown}
  onpointerdown={absorb}
  onpointerup={absorb}
  onpointermove={absorb}
  onclick={absorb}
  ondblclick={absorb}
  oncontextmenu={absorb}
  onwheel={absorb}
>
  <div class="summary">{@render summary()}</div>
  {#if children}
    <div class="tools" inert={!expanded}>
      <div class="tools-inner">{@render children()}</div>
    </div>
  {/if}
</div>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0;
    min-height: 48px;
    padding: 0 var(--space-1) 0 var(--space-5);
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    background: var(--surface-glass);
    -webkit-backdrop-filter: blur(18px) saturate(1.2);
    backdrop-filter: blur(18px) saturate(1.2);
    box-shadow: var(--shadow-float);
    color: var(--text);
    pointer-events: auto;
    transition:
      gap var(--dur-base) var(--ease-out),
      padding var(--dur-base) var(--ease-out);
  }
  .pill:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 3px;
  }
  .summary {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    min-height: 40px;
    padding-right: var(--space-3);
    font-size: var(--fs-body);
    font-weight: 500;
    white-space: nowrap;
  }
  /* grid 0fr → 1fr animates the width without measuring */
  .tools {
    display: grid;
    grid-template-columns: 0fr;
    opacity: 0;
    transition:
      grid-template-columns var(--dur-base) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }
  .tools-inner {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
    overflow: hidden;
    /* room for the children's focus outlines inside the clip */
    padding: 6px 6px 6px var(--space-3);
    border-left: 1px solid var(--line);
  }
  .expanded .tools {
    grid-template-columns: 1fr;
    opacity: 1;
    transition-delay: 0ms, 80ms;
  }
</style>
