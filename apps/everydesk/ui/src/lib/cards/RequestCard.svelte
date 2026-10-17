<!--
  RequestCard — the shell shared by the access request and the "still connected?" ring
  (spec 6.2 moment 4: same component, different words).
  Props:
    name: string — who is asking (large type)
    line: string — what they want ("quer ver e controlar a sua tela")
    code?: string — their machine code, shown small in mono
    total: number — seconds of the whole window (60, or ring_secs)
    remaining: number — seconds left; the accent ring around the initials shrinks with it
      (the ring is the only clock: no separate seconds counter; it reads the time to screen
      readers as a progressbar)
    offset?: number — px inset from the right edge when cards stack (cards.ts stackOffset)
    children?: Snippet — identity line, "lembrar" controls, error
    actions: Snippet — the two buttons
  Slides in from the corner (--dur-base, --ease-out; 0 ms under reduced motion) and fades out.
  It is a non-modal alertdialog (name = who, description = what they want); IncomingCard
  announces it through its live region and moves the focus to the button marked
  `data-autofocus`.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import { fade } from 'svelte/transition'
  import '../../design/tokens.css'
  import '../../design/typography.css'
  import { DUR, prefersReducedMotion } from '../../design/motion'
  import Ring from '../ui/Ring.svelte'
  import { initials } from './cards'

  interface Props {
    name: string
    line: string
    code?: string
    total: number
    remaining: number
    offset?: number
    children?: Snippet
    actions: Snippet
  }
  let { name, line, code, total, remaining, offset = 0, children, actions }: Props = $props()

  const uid = $props.id()
  const letters = $derived(initials(name))
</script>

<div
  class="card"
  role="alertdialog"
  style:margin-right="{offset}px"
  aria-labelledby="n-{uid}"
  aria-describedby="l-{uid}"
  out:fade|global={{ duration: prefersReducedMotion() ? 0 : DUR.fast }}
>
  <header>
    <Ring {total} {remaining} size={76} stroke={3} name="Tempo para responder" label="{remaining} segundos para responder">
      <span class="disc" aria-hidden="true">{letters}</span>
    </Ring>
    <div class="who">
      <h2 id="n-{uid}" class="name">{name}</h2>
      <p id="l-{uid}" class="line t-body">{line}</p>
      {#if code}<p class="code t-body">código <span class="t-num">{code}</span></p>{/if}
    </div>
  </header>

  {#if children}<div class="body">{@render children()}</div>{/if}

  <div class="actions">{@render actions()}</div>
</div>

<style>
  .card {
    position: relative;
    width: min(440px, calc(100vw - 2 * var(--space-4)));
    box-sizing: border-box;
    display: grid;
    gap: var(--space-5);
    padding: var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-glass);
    -webkit-backdrop-filter: blur(20px) saturate(1.2);
    backdrop-filter: blur(20px) saturate(1.2);
    box-shadow: var(--shadow-float);
    color: var(--text);
    font-family: var(--font-ui);
    animation: arrive var(--dur-base) var(--ease-out) both;
  }
  /* from the corner the stack lives in (bottom right) */
  @keyframes arrive {
    from {
      opacity: 0;
      transform: translate(24px, 24px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  header {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--space-4);
  }
  .disc {
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 1.375rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .who {
    min-width: 0;
    display: grid;
    gap: var(--space-1);
  }
  .name {
    margin: 0;
    font-size: 1.75rem;
    font-weight: 600;
    line-height: var(--lh-tight);
    letter-spacing: var(--track-title);
    overflow-wrap: anywhere;
  }
  .line {
    margin: 0;
    color: var(--text);
    line-height: var(--lh-snug);
  }
  /* the code is security information: full text colour, not a muted caption */
  .code {
    margin: 0;
    color: var(--text);
  }

  .body {
    display: grid;
    gap: var(--space-4);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  /* "danger ghost": the danger button without its fill, until hover */
  .actions :global(.btn.danger:not(:hover)) {
    background: transparent;
  }
</style>
