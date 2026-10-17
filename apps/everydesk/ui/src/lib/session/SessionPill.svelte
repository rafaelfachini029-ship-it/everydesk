<!--
  SessionPill — the only chrome over the remote screen (spec §6.2 moment 3).
  Collapsed: latency, who is on the other side, the voice dot. Hover or focus expands it into
  the session tools; it hides 2 s after the pointer and focus leave, and comes back when the
  pointer reaches the top edge (peek) or on an Alt tap (summon, which also focuses it).
  Only what works is here (sessionPill.ts PILL_TOOLS): monitors, Arquivos (the system picker;
  dropping on the screen sends too), the quality reading and Encerrar. File sends show as a
  line of light along the pill's lower edge (all of them together) and as the FileTrail rows
  under it; while the trail has anything to show the pill stays up. On narrow windows (or
  200 % zoom) the pill wraps onto two rows so Encerrar is never clipped.
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
  import PhoneOff from 'lucide-svelte/icons/phone-off'
  import FileUp from 'lucide-svelte/icons/file-up'
  import Button from '../ui/Button.svelte'
  import Icon from '../ui/Icon.svelte'
  import Pill from '../ui/Pill.svelte'
  import FileTrail from './FileTrail.svelte'
  import MorePanel from './MorePanel.svelte'
  import ChatPanel from './ChatPanel.svelte'
  import VoiceDot from './VoiceDot.svelte'
  import VoiceTool from './VoiceTool.svelte'
  import RecordLamp from './RecordLamp.svelte'
  import RecordTool from './RecordTool.svelte'
  import { recording } from './record.ts'
  import { VIEW_ONLY_FILES, filesAllowed, qualityLabel } from './more.ts'
  import type { Contact } from '../policy.ts'
  import { currentDisplay, dispatchJobs, displayList, extras, jobs, pickAndSend, quality, switchDisplay } from '../session'
  import { overall, trailBusy } from './jobs.ts'
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
  const level = $derived(qualityLabel($extras.quality))
  // Só olhar sends no files either (the Arquivos tool reads as unavailable)
  const sends = $derived(filesAllowed($extras))
  const filesHint = $derived(sends ? 'Enviar arquivos para a outra máquina' : VIEW_ONLY_FILES)
  const fps = $derived(formatFps($quality.fps))

  const busy = $derived(trailBusy($jobs))
  const progress = $derived(overall($jobs.jobs))

  const hideTimer = createHideTimer(() => {
    // a send in the trail keeps the pill up until it is over
    if (busy) return hideTimer.poke()
    // recording: the lamp stays in sight (collapsed), spec §5.3
    if ($recording.on) return ((expanded = false), (hint = null), hideTimer.poke())
    shown = false
    expanded = false
    hint = null
  })
  onDestroy(() => hideTimer.cancel())
  $effect(() => {
    if (busy) {
      shown = true
      hideTimer.poke()
    }
  })

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
  function sendSome() {
    onreturn()
    pickAndSend().catch((e) => dispatchJobs({ type: 'refused', message: String(e), at: Date.now() }))
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
  class:busy
  inert={!shown}
  role="presentation"
  onpointerenter={() => hideTimer.hold()}
  onpointerleave={() => hideTimer.release()}
  {onfocusin}
  {onfocusout}
  {onkeydowncapture}
  ontransitionend={stale}
>
  <div class="bar">
    <div class="pillwrap">
      <Pill label="Controles da sessão" bind:expanded>
        {#snippet summary()}
          <span class="lat t-num" aria-label="Atraso {latency}">{latency}</span>
          {#if name}
            <span class="who">{name}</span>
          {:else}
            <span class="who code t-num">{peerId}</span>
          {/if}
          <VoiceDot />
          <RecordLamp />
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
          class="tool"
          aria-label="Arquivos"
          aria-disabled={sends ? undefined : 'true'}
          aria-describedby={sends ? undefined : `${uid}-look`}
          onclick={() => (sends ? sendSome() : onreturn())}
          onpointerenter={() => show(filesHint)}
          onfocus={() => show(filesHint)}
          onpointerleave={() => show(null)}
        >
          <Icon icon={FileUp} />
        </button>
        <RecordTool {onreturn} onhint={show} onattention={(on) => (on ? ((expanded = shown = true), hideTimer.hold()) : hideTimer.release())} />

        <span class="quality">
          <span class="glyph"><Icon icon={Gauge} /></span>
          <span class="auto">{level}</span>
          {#if fps}<span class="fps t-num" aria-hidden="true">{fps}</span>{/if}
          <span class="sr-only">Qualidade {level}{fps ? `, ${fps.replace('fps', 'quadros por segundo')}` : ''}</span>
        </span>

        <VoiceTool {onreturn} onhint={show} onattention={(on) => (on ? ((expanded = shown = true), hideTimer.hold()) : hideTimer.release())} />
        <ChatPanel active={expanded} {onreturn} who={name ?? peerId} onattention={(on) => (on ? ((expanded = shown = true), hideTimer.hold()) : hideTimer.release())} />
        <MorePanel active={expanded} {onreturn} />
        <span class="rule" aria-hidden="true"></span>
        <Button variant="danger" icon={PhoneOff} onclick={onend}>Encerrar</Button>
      </Pill>

      <span id="{uid}-one" hidden>A outra máquina tem um monitor só</span>
      <span id="{uid}-look" hidden>{VIEW_ONLY_FILES}</span>
      <span class="sendline" class:on={progress !== null} style:--p={progress ?? 0} aria-hidden="true"
        ><span></span></span
      >
    </div>
    <p class="hint" class:on={expanded && hint} aria-hidden="true">{hint ?? ''}</p>
  </div>
  <FileTrail {onreturn} />
</div>

<style>
  .dock {
    position: absolute;
    top: var(--space-3);
    left: 50%;
    z-index: 2;
    display: flex;
    flex-direction: column;
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
  /* the trail's rows need room; only while there is something to show */
  .dock.busy {
    min-width: min(340px, calc(100vw - 2 * var(--space-4)));
  }
  .bar {
    position: relative;
    display: flex;
    justify-content: center;
  }
  .pillwrap {
    position: relative;
    display: flex;
    max-width: 100%;
  }
  /* All sends together, as one line of light along the pill's lower edge. */
  .sendline {
    position: absolute;
    left: var(--space-5);
    right: var(--space-5);
    bottom: 0;
    height: 2px;
    border-radius: 1px;
    background: var(--accent-track);
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--dur-slow) var(--ease-out);
  }
  .sendline.on {
    opacity: 1;
  }
  .sendline span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent-glow);
    transform-origin: left center;
    transform: scaleX(var(--p));
    transition: transform var(--dur-slow) var(--ease-out);
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

  /* Narrow window or 200 % zoom: the tools wrap under the summary instead of being clipped,
     so Encerrar always stays on screen. */
  @media (max-width: 720px) {
    .dock :global(.pill) {
      flex-wrap: wrap;
      justify-content: center;
      border-radius: var(--radius-lg);
      padding: var(--space-1) var(--space-2);
    }
    .dock :global(.pill.expanded .tools) {
      flex-basis: 100%;
    }
    .dock :global(.pill .tools-inner) {
      flex-wrap: wrap;
      justify-content: center;
      row-gap: var(--space-1);
      border-left: 0;
      padding-left: 6px;
    }
    .dock :global(.pill.expanded .tools-inner) {
      border-top: 1px solid var(--line);
    }
    .rule {
      display: none;
    }
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
  .quality {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2) 0 var(--space-1);
    white-space: nowrap;
  }
  .quality .glyph {
    padding: 0;
  }
  .auto {
    font-size: var(--fs-body);
    color: var(--text);
  }
  .fps {
    font-size: var(--fs-body);
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
