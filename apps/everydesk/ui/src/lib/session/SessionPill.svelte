<!--
  SessionPill — the only chrome over the remote screen (spec §6.2 moment 3).
  Collapsed: latency, who is on the other side, the voice dot. Hover or focus expands it into
  the session tools; it hides 2 s after the pointer and focus leave, and comes back when the
  pointer reaches the top edge (peek) or on an Alt tap (summon, which also focuses it).
  Props:
    peerId: string
    onend: () => void — "Encerrar"
    onreturn: () => void — give the keyboard back to the remote screen (Escape, after a tool)
    onenter?: () => void — focus moved into the pill (release keys the peer still holds)
  Exports: peek(), summon(), hitRect() (null while hidden), owns(node).
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onDestroy, onMount, tick } from 'svelte'
  import Monitor from 'lucide-svelte/icons/monitor'
  import Gauge from 'lucide-svelte/icons/gauge'
  import MessagesSquare from 'lucide-svelte/icons/messages-square'
  import Folder from 'lucide-svelte/icons/folder'
  import CircleDot from 'lucide-svelte/icons/circle-dot'
  import Mic from 'lucide-svelte/icons/mic'
  import PhoneOff from 'lucide-svelte/icons/phone-off'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import Pill from '../ui/Pill.svelte'
  import FileTrail from './FileTrail.svelte'
  import type { Contact } from '../policy.ts'
  import { currentDisplay, displayList, quality, switchDisplay } from '../session'
  import { createHideTimer, formatFps, formatLatency, type Rect } from './sessionPill.ts'

  interface Props {
    peerId: string
    onend: () => void
    onreturn: () => void
    onenter?: () => void
  }
  let { peerId, onend, onreturn, onenter }: Props = $props()

  const uid = $props.id()
  let dock: HTMLDivElement
  let shown = $state(true)
  let expanded = $state(false)
  let name = $state<string | null>(null)
  let hint = $state<string | null>(null)

  const latency = $derived(formatLatency($quality.delay))
  const fps = $derived(formatFps($quality.fps))
  const SOON = 'em breve'
  const soon = [
    { icon: MessagesSquare, label: 'Conversa' },
    { icon: Folder, label: 'Arquivos' },
    { icon: CircleDot, label: 'Gravar' },
    { icon: Mic, label: 'Voz' },
  ]

  const hideTimer = createHideTimer(() => {
    shown = false
    expanded = false
    hint = null
  })
  onDestroy(() => hideTimer.cancel())

  onMount(() => {
    hideTimer.poke()
    invoke<Contact[]>('contacts_list')
      .then((list) => (name = list?.find((c) => c.peer_id === peerId)?.name ?? null))
      .catch(() => {})
  })

  /** Show it (collapsed) and restart the hide countdown. */
  export function peek() {
    shown = true
    hideTimer.poke()
  }
  /** Alt tap: show it and take the keyboard (it expands on focus). */
  export function summon() {
    shown = true
    tick().then(() => dock.querySelector<HTMLElement>('.pill')?.focus())
  }
  // The rect is read on every pointer event: cache it, drop it whenever it can move
  // (size change, end of a show/hide/expand transition, window resize, shown toggles).
  let rect: Rect | null = null
  const stale = () => (rect = null)
  $effect(() => {
    void shown
    stale()
    const ro = new ResizeObserver(stale)
    ro.observe(dock)
    window.addEventListener('resize', stale)
    return () => {
      ro.disconnect()
      window.removeEventListener('resize', stale)
    }
  })
  /** Where pointer input must not reach the remote; null while hidden. */
  export function hitRect(): Rect | null {
    return shown ? (rect ??= dock.getBoundingClientRect()) : null
  }
  export function owns(node: Node | null): boolean {
    return !!node && dock.contains(node)
  }

  function onfocusin() {
    shown = true
    hideTimer.hold()
    onenter?.()
  }
  function onfocusout(e: FocusEvent) {
    if (!dock.contains(e.relatedTarget as Node | null)) hideTimer.release()
  }
  // Capture: runs before Pill's own Escape handler, which still collapses it.
  function onkeydowncapture(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      expanded = false
      hint = null
      onreturn()
    }
  }
  function show(text: string | null) {
    hint = text
  }
  function pick(i: number) {
    switchDisplay(i)
    onreturn()
  }
</script>

<div
  bind:this={dock}
  class="dock"
  class:hidden={!shown}
  inert={!shown}
  role="presentation"
  onpointerenter={() => hideTimer.hold()}
  onpointerleave={() => hideTimer.release()}
  {onfocusin}
  {onfocusout}
  {onkeydowncapture}
  ontransitionend={stale}
>
  <Pill label="Controles da sessão" bind:expanded>
    {#snippet summary()}
      <span class="lat t-num" aria-label="Atraso {latency}">{latency}</span>
      {#if name}
        <span class="who">{name}</span>
      {:else}
        <span class="who code t-num">{peerId}</span>
      {/if}
      <span class="voice" role="img" aria-label="Voz desligada"></span>
    {/snippet}

    <div class="group" role="group" aria-label="Monitores">
      {#if $displayList.length > 1}
        <span class="glyph"><Icon icon={Monitor} /></span>
        {#each $displayList as _, i (i)}
          <button
            class="tool num t-num"
            aria-pressed={i === $currentDisplay}
            aria-label="Monitor {i + 1} de {$displayList.length}"
            onclick={() => pick(i)}
            onpointerenter={() => show(`Monitor ${i + 1} de ${$displayList.length}`)}
            onfocus={() => show(`Monitor ${i + 1} de ${$displayList.length}`)}
            onpointerleave={() => show(null)}>{i + 1}</button
          >
        {/each}
      {:else}
        <button
          class="tool"
          aria-disabled="true"
          onclick={onreturn}
          aria-label="Monitores"
          aria-describedby="{uid}-one"
          onpointerenter={() => show('Um monitor só')}
          onfocus={() => show('Um monitor só')}
          onpointerleave={() => show(null)}
        >
          <Icon icon={Monitor} />
        </button>
      {/if}
    </div>

    <button
      class="tool wide"
      aria-disabled="true"
          onclick={onreturn}
      aria-label="Qualidade automática"
      aria-describedby="{uid}-soon"
      onpointerenter={() => show(`Qualidade · ${SOON}`)}
      onfocus={() => show(`Qualidade · ${SOON}`)}
      onpointerleave={() => show(null)}
    >
      <Icon icon={Gauge} />
      <span class="auto">automática</span>
    </button>
    {#if fps}<span class="fps t-num" aria-label="{fps} por segundo">{fps}</span>{/if}

    <span class="rule" aria-hidden="true"></span>

    {#each soon as t (t.label)}
      <button
        class="tool"
        aria-disabled="true"
          onclick={onreturn}
        aria-label={t.label}
        aria-describedby="{uid}-soon"
        onpointerenter={() => show(`${t.label} · ${SOON}`)}
        onfocus={() => show(`${t.label} · ${SOON}`)}
        onpointerleave={() => show(null)}
      >
        <Icon icon={t.icon} />
      </button>
    {/each}

    <span class="rule" aria-hidden="true"></span>
    <Button variant="danger" icon={PhoneOff} onclick={onend}>Encerrar</Button>
  </Pill>

  <span id="{uid}-soon" hidden>{SOON}</span>
  <span id="{uid}-one" hidden>A outra máquina tem um monitor só</span>
  <p class="hint" class:on={expanded && hint} aria-hidden="true">{hint ?? ''}</p>
  <FileTrail />
</div>

<style>
  .dock {
    position: absolute;
    top: var(--space-3);
    left: 50%;
    z-index: 2;
    display: flex;
    justify-content: center;
    width: max-content;
    max-width: calc(100vw - 2 * var(--space-4));
    transform: translate(-50%, 0);
    opacity: 1;
    /* transform + opacity only: composited, never re-lays out the screen below */
    transition:
      transform var(--dur-base) var(--ease-out),
      opacity var(--dur-base) var(--ease-out),
      visibility 0s linear 0s;
  }
  .dock.hidden {
    transform: translate(-50%, calc(-100% - var(--space-5)));
    opacity: 0;
    visibility: hidden;
    transition:
      transform var(--dur-base) var(--ease-out),
      opacity var(--dur-base) var(--ease-out),
      visibility 0s linear var(--dur-base);
  }
  /* A hidden pill must never catch input, even while it fades. */
  .dock.hidden :global(.pill) {
    pointer-events: none;
  }
  /* No backdrop-filter over a canvas that repaints every frame: an opaque surface costs
     one plain composite instead of a blur per frame. */
  .dock :global(.pill) {
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
    background: var(--surface);
    border-color: var(--line-strong);
    max-width: 100%;
  }
  /* collapsed: the empty tool strip keeps no padding, so the summary sits centred */
  .dock :global(.pill:not(.expanded) .tools-inner) {
    padding-inline: 0;
    border-left-color: transparent;
  }

  .lat {
    min-width: 6ch;
    font-size: var(--fs-label);
    color: var(--text);
    letter-spacing: -0.01em;
  }
  .who {
    max-width: 18ch;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }
  .who.code {
    font-size: var(--fs-label);
    color: var(--text-muted);
  }
  .voice {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--line-strong);
  }

  .group {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .glyph {
    display: inline-flex;
    padding: 0 var(--space-1) 0 var(--space-2);
    color: var(--text-muted);
  }
  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    width: 40px;
    height: 40px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .tool:hover:not([aria-disabled='true']) {
    background: var(--surface-2);
  }
  .tool:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
  .tool[aria-disabled='true'] {
    color: var(--text-muted);
    cursor: not-allowed;
  }
  .tool.num {
    font-size: var(--fs-label);
    width: 32px;
    height: 32px;
  }
  .tool.num[aria-pressed='true'] {
    border-color: var(--line-strong);
    background: var(--surface-2);
  }
  .tool.wide {
    width: auto;
    padding: 0 var(--space-3);
  }
  .auto {
    font-size: var(--fs-label);
  }
  .fps {
    padding: 0 var(--space-2) 0 0;
    font-size: var(--fs-label);
    color: var(--text-muted);
  }
  .rule {
    width: 1px;
    height: 24px;
    margin: 0 var(--space-2);
    background: var(--line);
  }

  /* outside the flow so the dock's rect (the dead zone) is the pill alone */
  .hint {
    position: absolute;
    top: 100%;
    margin: var(--space-2) 0 0;
    padding: 2px var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text-muted);
    font-size: var(--fs-label);
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .hint.on {
    opacity: 1;
  }
</style>
